use crate::eval::sep_evaluator::SeparationEvaluator;
use crate::quantify::tracker::CollisionTracker;
use crate::sample::search;
use crate::sample::search::SampleConfig;
use crate::util::assertions::tracker_matches_layout;
use crate::util::terminator::Terminator;
use crate::FMT;
use itertools::Itertools;
use jagua_rs::entities::{Instance, PItemKey};
use jagua_rs::probs::spp::entities::{SPInstance, SPPlacement, SPProblem, SPSolution};
use jagua_rs::geometry::DTransformation;
use log::debug;
use rand::prelude::{SliceRandom, SmallRng};
use std::iter::Sum;
use std::ops::AddAssign;
use tap::Tap;

pub struct SeparatorWorker {
    pub instance: SPInstance,
    pub prob: SPProblem,
    pub ct: CollisionTracker,
    pub rng: SmallRng,
    pub sample_config: SampleConfig,
}

impl SeparatorWorker {
    pub fn load(&mut self, sol: &SPSolution, ct: &CollisionTracker) {
        // restores the state of the worker to the given solution and accompanying tracker
        debug_assert!(sol.strip_width() == self.prob.strip_width());
        self.prob.restore(sol);
        self.ct = ct.clone();
    }

    pub fn move_colliding_items(&mut self) -> SepStats {
        //collect all colliding items and shuffle them
        let candidates = self.prob.layout.placed_items.keys()
            .filter(|pk| self.ct.get_loss(*pk) > 0.0)
            .collect_vec()
            .tap_mut(|v| v.shuffle(&mut self.rng));

        let mut total_moves = 0;
        let mut total_evals = 0;

        //give each item a chance to move to a better (eval) position
        for &pk in candidates.iter() {
            //check if the item is still colliding
            if self.ct.get_loss(pk) > 0.0 {
                let item_id = self.prob.layout.placed_items[pk].item_id;
                let item = self.instance.item(item_id);

                // create an evaluator to evaluate the samples during the search
                let evaluator = SeparationEvaluator::new(&self.prob.layout, item, pk, &self.ct);

                // search for a better position for the item
                let (best_sample, n_evals) =
                    search::search_placement(&self.prob.layout, item, Some(pk), evaluator, self.sample_config, &mut self.rng);

                let (new_dt, _eval) = best_sample.expect("search_placement should always return a sample");

                // move the item to the new position
                self.move_item(pk, new_dt);
                total_moves += 1;
                total_evals += n_evals;
            }
        }
        SepStats { total_moves, total_evals }
    }

    pub fn move_item(&mut self, pk: PItemKey, d_transf: DTransformation) -> PItemKey {
        debug_assert!(tracker_matches_layout(&self.ct, &self.prob.layout));

        let item = self.instance.item(self.prob.layout.placed_items[pk].item_id);

        let (old_l, old_w_l) = (self.ct.get_loss(pk), self.ct.get_weighted_loss(pk));

        //modify the problem, by removing the item and placing it in the new position
        let old_placement = self.prob.remove_item(pk);
        let new_placement = SPPlacement { d_transf, item_id: item.id };
        let new_pk = self.prob.place_item(new_placement);
        //update the collision tracker to reflect the changes
        self.ct.register_item_move(&self.prob.layout, pk, new_pk);

        let (new_l, new_w_l) = (self.ct.get_loss(new_pk), self.ct.get_weighted_loss(new_pk));

        debug!("Moved {:?} (l: {}, wl: {}) to {:?} (l+1: {}, wl+1: {})", old_placement, FMT().fmt2(old_l), FMT().fmt2(old_w_l), new_placement, FMT().fmt2(new_l), FMT().fmt2(new_w_l));
        debug_assert!(new_w_l <= old_w_l * 1.001, "weighted loss should never increase: {} > {}", FMT().fmt2(old_w_l), FMT().fmt2(new_w_l));
        debug_assert!(tracker_matches_layout(&self.ct, &self.prob.layout));

        new_pk
    }
}

pub struct SepStats {
    pub total_moves: usize,
    pub total_evals: usize,
}

impl Sum for SepStats {
    fn sum<I: Iterator<Item=SepStats>>(iter: I) -> Self {
        let mut total_moves = 0;
        let mut total_evals = 0;

        for report in iter {
            total_moves += report.total_moves;
            total_evals += report.total_evals;
        }

        SepStats { total_moves, total_evals }
    }
}

impl AddAssign for SepStats {
    fn add_assign(&mut self, other: Self) {
        self.total_moves += other.total_moves;
        self.total_evals += other.total_evals;
    }
}
