use crate::eval::sample_eval::{SampleEval, SampleEvaluator};
use crate::eval::specialized_jaguars_pipeline::{collect_poly_collisions_in_detector_custom, SpecializedDetectionMap};
use jagua_rs::collision_detection::hazard_helpers::HazardDetector;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use crate::quantify::tracker::CollisionTracker;

pub struct SeparationEvaluator<'a> {
    layout: &'a Layout,
    item: &'a Item,
    detection_map: SpecializedDetectionMap<'a>,
    shape_buff: SimplePolygon,
    n_evals: usize,
}

impl<'a> SeparationEvaluator<'a> {
    pub fn new(
        layout: &'a Layout,
        item: &'a Item,
        current_pk: PItemKey,
        ct: &'a CollisionTracker,
    ) -> Self {
        let detection_map = SpecializedDetectionMap::new(layout, ct, current_pk);

        Self {
            layout,
            item,
            detection_map,
            shape_buff: item.shape.as_ref().clone(),
            n_evals: 0,
        }
    }
}

impl<'a> SampleEvaluator for SeparationEvaluator<'a> {
    fn eval(&mut self, dt: DTransformation, upper_bound: Option<SampleEval>) -> SampleEval {
        self.n_evals += 1;
        let cde = self.layout.cde();

        // evals with higher loss than this will always be rejected
        let loss_bound = match upper_bound {
            Some(SampleEval::Collision { loss }) => loss,
            Some(SampleEval::Clear { .. }) => 0.0,
            _ => fsize::INFINITY,
        };
        // reload the detection map for the new query and update its loss bound
        self.detection_map.reload(loss_bound);

        //query the CDE for collisions and eval them
        collect_poly_collisions_in_detector_custom(cde, &dt, &mut self.shape_buff, self.item.shape.as_ref(), &mut self.detection_map);

        if self.detection_map.is_empty() {
            SampleEval::Clear { loss: 0.0 }
        } else if self.detection_map.early_terminate(&self.shape_buff) {
            //the early termination was triggered, this means potentially not all collisions were detected,
            //but the sample will be rejected anyway.
            SampleEval::Invalid
        } else {
            let loss = self.detection_map.loss(&self.shape_buff);
            SampleEval::Collision { loss }
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}

