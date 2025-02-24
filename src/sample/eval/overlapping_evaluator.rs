use crate::overlap::overlap_proxy::{bin_overlap_proxy, poly_overlap_proxy};
use crate::overlap::tracker::OverlapTracker;
use crate::sample::eval::{SampleEval, SampleEvaluator};
use float_cmp::approx_eq;
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
        self.n_evals += 1;
        let cde = self.layout.cde();

        self.coll_buff.clear();
        let transf = dt.compose();
        self.shape_buff.transform_from(&self.item.shape, &transf);
        let irrel_haz = HazardEntity::from(&self.layout.placed_items[self.current_pk]);

        //do a check with the surrogate, calculate overlap
        cde.collect_surrogate_collisions_in_buffer(&self.item.shape.surrogate(), &transf, &[irrel_haz], &mut self.coll_buff);

        //calculate weighted overlap for all hazards detected by the surrogate
        let surr_w_overlap = self.calc_overlap_cost(&self.coll_buff);

        //if this already exceeds the upperbound, return
        if let Some(SampleEval::Colliding(upper_bound)) = upper_bound {
            if surr_w_overlap > upper_bound {
                debug_assert!(self.eval(dt, None) > SampleEval::Colliding(upper_bound));
                return SampleEval::Invalid;
            }
        }

        //If not, move onto a full collision check
        let n_detected_by_surr = self.coll_buff.len();
        cde.collect_poly_collisions_in_buffer(&self.shape_buff, &[irrel_haz], &mut self.coll_buff);

        //By now, the buffer should contain all hazards
        if self.coll_buff.is_empty() {
            SampleEval::Valid(0.0)
        } else {
            //Calculate the remaining weighted overlap for the hazards not detected by the surrogate
            let extra_hazards = &self.coll_buff[n_detected_by_surr..];
            let full_w_overlap = surr_w_overlap + self.calc_overlap_cost(extra_hazards);

            debug_assert!(approx_eq!(fsize, full_w_overlap, self.calc_overlap_cost(&self.coll_buff)));

            SampleEval::Colliding(full_w_overlap)
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}

impl<'a> OverlappingSampleEvaluator<'a> {
    pub fn calc_overlap_cost(&self, colliding: &[HazardEntity]) -> fsize {
        colliding.iter().map(|haz| match haz {
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
        }).sum()
    }
}