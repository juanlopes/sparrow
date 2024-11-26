use std::time::Duration;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::instances::strip_packing::SPInstance;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::problem_generic::{ProblemGeneric, STRIP_LAYOUT_IDX};
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use rand::rngs::SmallRng;
use crate::overlap::overlap_tracker::OverlapTracker;
use crate::sampl::evaluator::SampleEval;

const N_UNIFORM: usize = 100;
const N_CD: usize = 2;

const TIME_LIMIT: Duration = Duration::from_secs(60);

pub struct GLSOptimizer{
    pub prob: SPProblem,
    pub instance: SPInstance,
    pub rng: SmallRng,
    pub ot: OverlapTracker,
}


impl GLSOptimizer {
    pub fn new(prob: SPProblem, instance: SPInstance, rng: SmallRng) -> Self {
        let ot = OverlapTracker::new(instance.total_item_qty());
        Self {
            prob,
            instance,
            rng,
            ot,
        }
    }

    pub fn solve(&mut self) -> Solution {
        todo!()
    }

    pub fn rollback(&mut self, solution: &Solution, ot: &OverlapTracker){
        todo!();
    }

    fn separate_layout(&mut self){
        todo!();
    }

    fn modify(&mut self){
        todo!();
    }

    fn place_item(&mut self, old_pk: Option<PItemKey>, new_p_opt: PlacingOption) -> PItemKey {
        match old_pk {
            Some(old_pk) => {
                self.prob.remove_item(STRIP_LAYOUT_IDX, old_pk, true);
                let (_, new_pk) = self.prob.place_item(new_p_opt);
                self.ot.move_item(&self.prob.layout, old_pk, new_pk);
                new_pk
            }
            None => {
                let (_, new_pk) = self.prob.place_item(new_p_opt);
                self.ot.sync(&self.prob.layout);
                new_pk
            }
        }
    }

    fn change_strip_width(&mut self, new_width: fsize) {
        todo!();
    }
}

