use float_cmp::{approx_eq, assert_approx_eq};
use crate::overlap::overlap_proxy::{bin_overlap_proxy, poly_overlap_proxy};
use crate::overlap::tracker::OverlapTracker;
use crate::sample::eval::{SampleEval, SampleEvaluator};
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::{Shape, TransformableFrom};
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;

const USE_NEW_EVAL: bool = true;

pub struct OverlappingSampleEvaluator<'a> {
    layout: &'a Layout,
    item: &'a Item,
    current_pk: PItemKey,
    ot: &'a OverlapTracker,
    coll_buff: Vec<HazardEntity>,
    shape_buff: SimplePolygon,
    n_evals: usize,
}

impl<'a> OverlappingSampleEvaluator<'a> {
    pub fn new(
        layout: &'a Layout,
        item: &'a Item,
        current_pk: PItemKey,
        ot: &'a OverlapTracker,
    ) -> Self {
        Self {
            layout,
            item,
            current_pk,
            ot,
            coll_buff: vec![],
            shape_buff: item.shape.as_ref().clone(),
            n_evals: 0,
        }
    }
}

impl<'a> SampleEvaluator for OverlappingSampleEvaluator<'a> {
    fn eval(&mut self, dt: DTransformation, upper_bound: Option<SampleEval>) -> SampleEval {
        if USE_NEW_EVAL {
            let new_eval = self.eval_new(dt, upper_bound);
            debug_assert!({
                let old_eval = self.eval_old(dt);
                if old_eval < upper_bound.unwrap_or(SampleEval::Invalid) {
                    //below upperbound
                    match (old_eval, new_eval){
                        (SampleEval::Valid(_), SampleEval::Valid(_)) => {},
                        (SampleEval::Invalid, SampleEval::Invalid) => {},
                        (SampleEval::Colliding(old_w_o), SampleEval::Colliding(new_w_o)) => {
                            assert_approx_eq!(fsize, old_w_o, new_w_o);
                        },
                        _ => assert!(false, "old_eval: {:?}, new_eval: {:?}, upperbound: {:?}", old_eval, new_eval, upper_bound),
                    }
                } else {
                    //above upperbound
                    //dbg!("above upperbound");
                    assert!(new_eval >= old_eval);
                }
                true
            }, "new eval: {:?}, old eval: {:?}, upperbound: {:?}, cmp: {:?}", new_eval, self.eval_old(dt), upper_bound, new_eval.cmp(&self.eval_old(dt)));
            new_eval
        } else {
            self.eval_old(dt)
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}

impl<'a> OverlappingSampleEvaluator<'a> {
    fn eval_old(&mut self, dt: DTransformation) -> SampleEval {
        self.n_evals += 1;
        let cde = self.layout.cde();

        self.coll_buff.clear();
        self.shape_buff.transform_from(&self.item.shape, &dt.into());

        let irrel_haz = HazardEntity::from(&self.layout.placed_items[self.current_pk]);

        cde.collect_poly_collisions_in_buffer(&self.shape_buff, &[irrel_haz], &mut self.coll_buff);

        if self.coll_buff.is_empty() {
            SampleEval::Valid(0.0)
        } else {
            let w_overlap = self.coll_buff.iter()
                .map(|haz| match haz {
                    HazardEntity::PlacedItem { .. } => {
                        let other_pk = self.layout.hazard_to_p_item_key(&haz).unwrap();
                        let other_shape = &self.layout.placed_items[other_pk].shape;
                        let overlap = poly_overlap_proxy(&self.shape_buff, other_shape);
                        let weight = self.ot.get_pair_weight(self.current_pk, other_pk);
                        overlap * weight
                    }
                    HazardEntity::BinExterior => {
                        let overlap = bin_overlap_proxy(&self.shape_buff, self.layout.bin.bbox());
                        let weight = self.ot.get_bin_weight(self.current_pk);
                        2.0 * overlap * weight
                    }
                    _ => unimplemented!("unsupported hazard entity"),
                })
                .sum();

            SampleEval::Colliding(w_overlap)
        }
    }

    fn eval_new(&mut self, dt: DTransformation, upper_bound: Option<SampleEval>) -> SampleEval {
        self.n_evals += 1;
        let cde = self.layout.cde();

        self.coll_buff.clear();
        let irrel_haz = HazardEntity::from(&self.layout.placed_items[self.current_pk]);
        let transf = dt.compose();
        self.shape_buff.transform_from(&self.item.shape, &transf);

        //do a check with the surrogate, calculate overlap
        cde.collect_surrogate_collisions_in_buffer(&self.item.shape.surrogate(), &transf, &[irrel_haz], &mut self.coll_buff);

        let w_overlap_surrogate = calc_overlap_cost(&self.coll_buff, self.layout, &self.shape_buff, self.current_pk, self.ot);

        if let Some(SampleEval::Colliding(upper_bound)) = upper_bound {
            if w_overlap_surrogate > upper_bound {
                return SampleEval::Invalid;
            }
        }

        let n_detected_surrogate = self.coll_buff.len();

        //do a full collision check
        cde.collect_poly_collisions_in_buffer(&self.shape_buff, &[irrel_haz], &mut self.coll_buff);
        if self.coll_buff.is_empty() {
            SampleEval::Valid(0.0)
        } else {
            let new_detected_slice = &self.coll_buff[n_detected_surrogate..];
            let w_overlap_rest = calc_overlap_cost(new_detected_slice, self.layout, &self.shape_buff, self.current_pk, self.ot);

            let full_w_overlap = w_overlap_surrogate + w_overlap_rest;

            debug_assert!(approx_eq!(fsize, full_w_overlap, calc_overlap_cost(&self.coll_buff, self.layout, &self.shape_buff, self.current_pk, self.ot)));

            SampleEval::Colliding(full_w_overlap)
        }
    }
}

fn calc_overlap_cost(colliding: &[HazardEntity], layout: &Layout, shape: &SimplePolygon, current_pk: PItemKey, ot: &OverlapTracker) -> fsize {
    colliding.iter().map(|haz| match haz {
        HazardEntity::PlacedItem { .. } => {
            let other_pk = layout.hazard_to_p_item_key(&haz).unwrap();
            let other_shape = &layout.placed_items[other_pk].shape;
            let overlap = poly_overlap_proxy(shape, other_shape);
            let weight = ot.get_pair_weight(current_pk, other_pk);
            overlap * weight
        }
        HazardEntity::BinExterior => {
            let overlap = bin_overlap_proxy(shape, layout.bin.bbox());
            let weight = ot.get_bin_weight(current_pk);
            2.0 * overlap * weight
        }
        _ => unimplemented!("unsupported hazard entity"),
    }).sum()
}