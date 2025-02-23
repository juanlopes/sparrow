use crate::overlap::overlap_proxy::{bin_overlap_proxy, poly_overlap_proxy};
use crate::overlap::tracker::OverlapTracker;
use crate::sample::eval::{SampleEval, SampleEvaluator};
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::{Shape, TransformableFrom};
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;

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
    pub fn new(layout: &'a Layout, item: &'a Item, current_pk: PItemKey, ot: &'a OverlapTracker) -> Self {
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
    fn eval(&mut self, dt: DTransformation) -> SampleEval {
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
                    _ => unimplemented!("unsupported hazard entity")
                }).sum();

            SampleEval::Colliding(w_overlap)
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}