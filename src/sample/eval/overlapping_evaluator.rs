use std::cmp::Ordering;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_enums::GeoRelation;
use jagua_rs::geometry::geo_traits::{Shape, Transformable, TransformableFrom};
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use jagua_rs::geometry::transformation::Transformation;
use jagua_rs::util::fpa::FPA;
use crate::overlap::overlap::{calculate_unweighted_overlap_shape, calculate_weighted_overlap};
use crate::overlap::overlap_tracker::OverlapTracker;
use crate::sample::eval::{SampleEval, SampleEvaluator};
use crate::sample::eval::hpg_eval::hpg_value;

pub struct OverlappingSampleEvaluator<'a> {
    layout: &'a Layout,
    item: &'a Item,
    current_pk: Option<PItemKey>,
    ot: &'a OverlapTracker,
    coll_buff: Vec<HazardEntity>,
    shape_buff: SimplePolygon,
    n_evals: usize,
}

impl<'a> OverlappingSampleEvaluator<'a> {
    pub fn new(layout: &'a Layout, item: &'a Item, current_pk: Option<PItemKey>, ot: &'a OverlapTracker) -> Self {
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

        // if self.shape_buff.bbox().relation_to(&self.layout.bin.bbox()) != GeoRelation::Enclosed {
        //     return SampleEval::Invalid;
        // }

        match self.current_pk {
            Some(current_pk) => {
                let current_pi = &self.layout.placed_items[current_pk];
                cde.collect_poly_collisions(&self.shape_buff, &[current_pi.into()], &mut self.coll_buff);
            }
            None => {
                panic!();
                cde.collect_poly_collisions(&self.shape_buff, &[], &mut self.coll_buff);
            }
        }

        if self.coll_buff.is_empty() {
            //let v = self.shape_buff.bbox.x_max + 0.1 * self.shape_buff.bbox.y_max;
            SampleEval::Valid(0.0)
        }
        else {
            let w_overlap = match self.current_pk {
                Some(current_pk) => {
                    calculate_weighted_overlap(self.layout, &self.shape_buff, current_pk, self.coll_buff.iter().cloned(), self.ot)
                }
                None => {
                    panic!();
                    calculate_unweighted_overlap_shape(self.layout, &self.shape_buff, self.coll_buff.iter().cloned())
                }
            };

            let hazard = self.current_pk.map(|pk| (&self.layout.placed_items[pk]).into());
            let hpg_value = hpg_value(self.layout.cde().haz_prox_grid().unwrap(), &self.shape_buff, hazard);
            SampleEval::Colliding { w_overlap, hpg_value }
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}