use crate::io;
use crate::io::layout_to_svg::{layout_to_svg, s_layout_to_svg};
use float_cmp::approx_eq;
use itertools::Itertools;
use jagua_rs::entities::bin::Bin;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::instances::strip_packing::SPInstance;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::problem_generic::{ProblemGeneric, STRIP_LAYOUT_IDX};
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::{Shape, Transformable};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::util::fpa::FPA;
use log::{info, warn};
use ordered_float::OrderedFloat;
use rand::prelude::{SliceRandom, SmallRng};
use std::char::decode_utf16;
use std::cmp::Reverse;
use std::collections::VecDeque;
use std::ops::Range;
use std::path::Path;
use std::time::Instant;
use tap::Tap;
use crate::io::svg_util::SvgDrawOptions;
use crate::overlap::overlap_tracker;
use crate::overlap::overlap_tracker::OverlapTracker;
use crate::sample::eval::overlapping_evaluator::OverlappingSampleEvaluator;
use crate::sample::eval::SampleEval;
use crate::sample::search;
use crate::sample::search::SearchConfig;

const N_ITER_NO_IMPROVEMENT: usize = 100;

const N_STRIKES: usize = 5;
const R_SHRINK: fsize = 0.005;
const R_EXPAND: fsize = 0.002;

const TIME_LIMIT_S: u64 = 5 * 60 * 60;

const N_UNIFORM_SAMPLES: usize = 100;
const N_COORD_DESCENTS: usize = 2;
const RESCALE_WEIGHT_TARGET: fsize = 10.0;

pub struct GLSOptimizer {
    pub problem: SPProblem,
    pub instance: SPInstance,
    pub rng: SmallRng,
    pub overlap_tracker: OverlapTracker,
    pub output_folder: String,
    pub svg_counter: usize,
    pub weight_multiplier: fsize,
}


impl GLSOptimizer {
    pub fn new(problem: SPProblem, instance: SPInstance, rng: SmallRng, output_folder: String) -> Self {
        let overlap_tracker = OverlapTracker::new(problem.instance.total_item_qty(), RESCALE_WEIGHT_TARGET);
        Self {
            problem,
            instance,
            rng,
            overlap_tracker,
            svg_counter: 0,
            output_folder,
            weight_multiplier: 1.0,
        }
    }

    pub fn solve(&mut self) -> Solution {
        let mut current_width = self.problem.occupied_width();
        let (mut best_feasible_solution, mut best_width) = (self.problem.create_solution(None), current_width);

        self.write_to_disk(None, true);

        let start = Instant::now();
        let mut i = 0;

        while start.elapsed().as_secs() < TIME_LIMIT_S {
            let (local_best, sol_tracker) = self.separate_layout();
            let total_overlap = self.overlap_tracker.get_total_overlap();
            let next_width = {
                if total_overlap == 0.0 {
                    //successful
                    if current_width < best_width {
                        warn!("new best width at: {:.3}", current_width);
                        best_width = current_width;
                        best_feasible_solution = local_best.clone();
                    }
                    current_width * (1.0 - R_SHRINK)
                } else {
                    //not successful
                    let expanded_width = current_width * (1.0 + R_EXPAND);
                    if expanded_width < best_width {
                        expanded_width
                    } else {
                        //current_width * (1.0 - R_SHRINK)
                        //expanded_width
                        //current_width * (1.0 - R_EXPAND)
                        //(current_width + best_width) / 2.0 //average between current and best
                        current_width
                    }
                }
            };
            self.overlap_tracker.rescale_weights();
            //self.overlap_tracker.halve_weights();
            self.write_to_disk(Some(local_best), true);
            warn!("width: {:.3} -> {:.3} (best: {:.3})", current_width, next_width, best_width);
            self.change_strip_width(next_width);
            current_width = next_width;
            i += 1;
        }
        best_feasible_solution
    }

    pub fn separate_layout(&mut self) -> (Solution, OverlapTracker) {
        let mut min_overlap = self.overlap_tracker.get_total_overlap();
        let mut min_overlap_solution = (self.problem.create_solution(None), self.overlap_tracker.clone());
        let initial_overlap = min_overlap;
        warn!("initial overlap: {:.3}", initial_overlap);

        let mut n_strikes = 0;

        while n_strikes < N_STRIKES {
            self.rollback(&min_overlap_solution.0, &min_overlap_solution.1);
            self.overlap_tracker.rescale_weights();
            let mut n_iter_no_improvement = 0;
            let mut improved = false;
            let mut total_movement = 0;

            let start = Instant::now();
            while n_iter_no_improvement < N_ITER_NO_IMPROVEMENT {
                let weighted_overlap_before = self.overlap_tracker.get_total_weighted_overlap();
                let n_movements = self.modify();
                let overlap = self.overlap_tracker.get_total_overlap();
                let weighted_overlap = self.overlap_tracker.get_total_weighted_overlap();
                debug_assert!(weighted_overlap <= weighted_overlap_before, "weighted overlap increased: {} -> {}", weighted_overlap_before, weighted_overlap);
                info!("[i:{}]  w_o-1: {:.3} -> {:.3}, n_mov: {:.3}, abs_o: {:.3} (min: {:.3})", n_iter_no_improvement, weighted_overlap_before, weighted_overlap, n_movements,overlap, min_overlap);
                if overlap == 0.0 {
                    warn!("separation successful, returning");
                    min_overlap_solution = (self.problem.create_solution(None), self.overlap_tracker.clone());
                    self.write_to_disk(None, false);
                    return min_overlap_solution;
                }
                if overlap < min_overlap {
                    min_overlap = overlap;
                    min_overlap_solution = (self.problem.create_solution(None), self.overlap_tracker.clone());
                    improved = true;
                    warn!("[i:{}]  w_o-1: {:.3} -> {:.3}, n_mov: {:.3}, abs_o: {:.3} (min: {:.3})", n_iter_no_improvement, weighted_overlap_before, weighted_overlap, n_movements,overlap, min_overlap);
                    n_iter_no_improvement = 0;
                    //self.overlap_tracker.rescale_weights();
                    //self.write_to_disk(None, true);
                } else {
                    n_iter_no_improvement += 1;
                }

                self.overlap_tracker.increment_weights();
                self.write_to_disk(None, false);
                total_movement += n_movements;
            }
            warn!("{:.1} moves/s", total_movement as f64 / start.elapsed().as_secs_f64());
            if !improved {
                n_strikes += 1;
            } else {
                n_strikes = 0;
            }
            warn!("strike: #{}", n_strikes);
        }

        if min_overlap > initial_overlap * 0.9 {
            warn!("not enough improvement, not restoring");
            // for _ in 0..100 {
            //     self.modify_solution();
            //     self.overlap_tracker.increment_weights();
            // }
        } else {
            warn!("improved from {:.3} to {:.3}, rolling back to min overlap", initial_overlap, min_overlap);
            self.rollback(&min_overlap_solution.0, &min_overlap_solution.1);
            self.overlap_tracker.rescale_weights();
            for i in 0..100 {
                self.overlap_tracker.increment_weights();
            }
        }

        min_overlap_solution
    }

    pub fn modify(&mut self) -> usize {

        let mut n_total_movements = 0;

        for i in 0..1 {
            let overlapping_items = self.problem.layout.placed_items()
                .keys()
                .filter(|&k| self.overlap_tracker.get_overlap(k) > 0.0)
                .collect_vec()
                .tap_mut(|v| v.shuffle(&mut self.rng));

            let mut n_movements = 0;

            for &pk in overlapping_items.iter() {
                let current_overlap = self.overlap_tracker.get_overlap(pk);
                if current_overlap > 0.0 {
                    let item = self.instance.item(self.problem.layout.placed_items()[pk].item_id);
                    let search_config = SearchConfig {
                        n_bin_samples: N_UNIFORM_SAMPLES / 2,
                        n_focussed_samples: N_UNIFORM_SAMPLES / 2,
                        n_coord_descents: N_COORD_DESCENTS,
                        n_valid_cutoff: None,
                    };

                    let evaluator = OverlappingSampleEvaluator::new(
                        &self.problem.layout,
                        item,
                        Some(pk),
                        &self.overlap_tracker
                    );

                    let new_placement = search::search_placement(
                        &self.problem.layout,
                        item,
                        Some(pk),
                        evaluator,
                        search_config,
                        &mut self.rng
                    );

                    self.move_item(pk, new_placement.0, Some(new_placement.1));
                    n_movements += 1;
                }
            }
            n_total_movements += n_movements;
            if n_movements == 0 {
                break;
            }
        }

        n_total_movements
    }

    pub fn change_strip_width(&mut self, new_width: fsize) {
        let current_width = self.problem.strip_width();
        let delta = new_width - current_width;
        //shift all items right of the center of the strip

        let shift_transf = DTransformation::new(0.0, (delta + FPA::tolerance(), 0.0));
        let items_to_shift = self.problem.layout.placed_items().iter()
            .filter(|(_, pi)| pi.shape.centroid().0 > current_width / 2.0)
            .map(|(k, pi)| (k, pi.d_transf))
            .collect_vec();

        for (pik, dtransf) in items_to_shift {
            let new_transf = dtransf.compose().translate(shift_transf.translation());
            self.move_item(pik, new_transf.decompose(), None);
        }

        let new_bin = Bin::from_strip(
            AARectangle::new(0.0, 0.0, new_width, self.problem.strip_height()),
            self.problem.layout.bin.base_cde.config().clone(),
        );
        self.problem.layout.change_bin(new_bin);
        info!("changed strip width to {}", new_width);
    }

    fn move_item(&mut self, pik: PItemKey, d_transf: DTransformation, eval: Option<SampleEval>) {
        debug_assert!(overlap_tracker::tracker_matches_layout(&self.overlap_tracker, &self.problem.layout));

        //Remove the item from the problem
        let old_p_opt = self.problem.remove_item(STRIP_LAYOUT_IDX, pik, true);
        let item = self.instance.item(old_p_opt.item_id);

        //Compute the colliding entities after the move
        let colliding_entities = {
            let shape = item.shape.clone().as_ref().clone()
                .tap_mut(|s| { s.transform(&d_transf.compose()); });

            let mut colliding_entities = vec![];
            self.problem.layout.cde().collect_poly_collisions(&shape, &[], &mut colliding_entities);
            colliding_entities
        };

        assert!({
            //make sure that if eval says it's valid, no colliding entities are detected
            if let Some(SampleEval::Valid(_)) = eval {
                colliding_entities.is_empty()
            } else {
                true
            }
        });

        let new_pik = {
            let new_p_opt = PlacingOption {
                d_transf,
                ..old_p_opt
            };

            let (_, new_pik) = self.problem.place_item(new_p_opt);
            new_pik
        };

        //info!("moving item {} from {:?} to {:?} ({:?}->{:?})", item.id, old_p_opt.d_transf, new_p_opt.d_transf, pik, new_pik);

        self.overlap_tracker.move_item(&self.problem.layout, pik, new_pik);

        debug_assert!(overlap_tracker::tracker_matches_layout(&self.overlap_tracker, &self.problem.layout));
    }

    pub fn write_to_disk(&mut self, solution: Option<Solution>, force: bool) {
        //make sure we are in debug mode or force is true
        if !force && !cfg!(debug_assertions) {
            return;
        }

        if self.svg_counter == 0 {
            //remove all .svg files from the output folder
            let _ = std::fs::remove_dir_all(&self.output_folder);
            std::fs::create_dir_all(&self.output_folder).unwrap();
        }


        match solution {
            Some(sol) => {
                let filename = format!("{}/{}_{:.2}_s.svg", &self.output_folder, self.svg_counter, self.problem.layout.bin.bbox().x_max);
                io::write_svg(
                    &s_layout_to_svg(&sol.layout_snapshots[0], &self.instance, SvgDrawOptions::default()),
                    Path::new(&filename),
                );
                warn!("wrote layout to disk: {}", filename);
            }
            None => {
                let filename = format!("{}/{}_{:.2}.svg", &self.output_folder, self.svg_counter, self.problem.layout.bin.bbox().x_max);
                io::write_svg(
                    &layout_to_svg(&self.problem.layout, &self.instance, SvgDrawOptions::default()),
                    Path::new(&filename),
                );
                warn!("wrote layout to disk: {}", filename);
            }
        }

        self.svg_counter += 1;
    }

    pub fn rollback(&mut self, solution: &Solution, ot: &OverlapTracker){
        self.problem.restore_to_solution(solution);
        self.overlap_tracker = ot.clone();
    }
}