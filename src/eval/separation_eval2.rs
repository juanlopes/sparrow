use crate::eval::custom_cde::{collect_poly_collisions_in_detector2, DetectionMap2};
use crate::eval::sample_eval::{SampleEval, SampleEvaluator};
use crate::overlap::tracker::OverlapTracker;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::collision_detection::hazard_helpers::HazardDetector;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::TransformableFrom;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use slotmap::SecondaryMap;

pub struct SeparationEvaluator2<'a> {
    layout: &'a Layout,
    item: &'a Item,
    current_pk: PItemKey,
    ot: &'a OverlapTracker,
    detection_map: DetectionMap2<'a>,
    shape_buff: SimplePolygon,
    n_evals: usize,
}

impl<'a> SeparationEvaluator2<'a> {
    pub fn new(
        layout: &'a Layout,
        item: &'a Item,
        current_pk: PItemKey,
        ot: &'a OverlapTracker,
    ) -> Self {
        let dt2 = DetectionMap2 {
            layout,
            ot,
            current_pk,
            detected_pis: SecondaryMap::new(),
            detected_bin: None,
            idx_counter: 0,
            weighted_overlap_cache: (0, 0.0),
            wo_upper_bound: 0.0,
        };

        Self {
            layout,
            item,
            current_pk,
            ot,
            detection_map: dt2,
            shape_buff: item.shape.as_ref().clone(),
            n_evals: 0,
        }
    }
}

impl<'a> SampleEvaluator for SeparationEvaluator2<'a> {
    fn eval(&mut self, dt: DTransformation, upper_bound: Option<SampleEval>) -> SampleEval {
        self.n_evals += 1;
        let cde = self.layout.cde();

        //prepare
        {
            let ub = match upper_bound {
                Some(SampleEval::Collision { loss }) => loss,
                Some(SampleEval::Clear { .. }) => 0.0,
                _ => fsize::INFINITY,
            };
            self.detection_map.reload(ub);
        }

        let transf = dt.compose();
        self.shape_buff.transform_from(&self.item.shape, &transf);

        collect_poly_collisions_in_detector2(cde, &self.shape_buff, &mut self.detection_map);

        if self.detection_map.len() == 0 {
            SampleEval::Clear { loss: 0.0 }
        } else {
            SampleEval::Collision {
                loss: self.detection_map.weighted_overlap(&self.shape_buff),
            }
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}