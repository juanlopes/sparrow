use crate::io::layout_to_svg::{layout_to_svg, s_layout_to_svg};
use crate::io::svg_util::SvgDrawOptions;
use crate::opt::tabu::TabuList;
use crate::overlap::overlap_tracker_original;
use crate::overlap::overlap_tracker_original::{OTSnapshot, OverlapTracker};
use crate::sample::eval::overlapping_evaluator::OverlappingSampleEvaluator;
use crate::sample::eval::SampleEval;
use crate::sample::search;
use crate::sample::search::SearchConfig;
use crate::{io, FMT, SVG_OUTPUT_DIR};
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
use log::{debug, info, warn};
use ordered_float::OrderedFloat;
use rand::distributions::{WeightedError, WeightedIndex};
use rand::prelude::{Distribution, IteratorRandom, SliceRandom, SmallRng};
use std::char::decode_utf16;
use std::cmp::Reverse;
use std::collections::VecDeque;
use std::iter;
use std::ops::Range;
use std::path::Path;
use std::time::{Duration, Instant};
use jagua_rs::geometry::geo_enums::GeoRelation;
use tap::Tap;

const N_ITER_NO_IMPROVEMENT: usize = 100;

const N_STRIKES: usize = 3;
const R_SHRINK: fsize = 0.005;
//const R_EXPAND: fsize = 0.003;

const N_UNIFORM_SAMPLES: usize = 100;
const N_COORD_DESCENTS: usize = 2;
const RESCALE_WEIGHT_TARGET: fsize = 10.0;

const WEIGHT_INCREMENT: fsize = 1.2;
const TABU_SIZE: usize = 100;
const JUMP_COOLDOWN: usize = 2;

pub struct GLSOptimizer {
    pub problem: SPProblem,
    pub instance: SPInstance,
    pub rng: SmallRng,
    pub overlap_tracker: OverlapTracker,
    pub output_folder: String,
    pub svg_counter: usize,
    pub tabu_list: TabuList,
}

impl GLSOptimizer {
    pub fn new(
        problem: SPProblem,
        instance: SPInstance,
        rng: SmallRng,
        output_folder: String,
    ) -> Self {
        let overlap_tracker = OverlapTracker::new(&problem.layout, RESCALE_WEIGHT_TARGET, WEIGHT_INCREMENT, JUMP_COOLDOWN);
        let tabu_list = TabuList::new(TABU_SIZE, &instance);
        Self {
            problem,
            instance,
            rng,
            overlap_tracker,
            svg_counter: 0,
            output_folder,
            tabu_list,
        }
    }

    pub fn solve(&mut self, time_out: Duration) -> Solution {
        //self.change_strip_width(5750.0);
        let mut current_width = self.problem.occupied_width();
        let (mut best_feasible_solution, mut best_width) = (self.problem.create_solution(None), current_width);


        self.write_to_disk(None, true);

        let start = Instant::now();
        let mut i = 0;

        while start.elapsed() < time_out {
            let local_best = self.separate_layout();
            let total_overlap = self.overlap_tracker.get_total_overlap();
            let next_width = {
                if total_overlap == 0.0 {
                    //successful
                    if current_width < best_width {
                        warn!("new best width at: {:.3}", current_width);
                        best_width = current_width;
                        best_feasible_solution = local_best.0.clone();
                    }
                    current_width * (1.0 - R_SHRINK)
                } else {
                    //not successful
                    self.rollback(&local_best.0, &local_best.1);
                    self.tabu_list.push(local_best.0.clone());
                    warn!("rolling back to local best and adding it to tabu list");
                    // let expanded_width = current_width * (1.0 + R_EXPAND);
                    // if expanded_width < best_width {
                    //     expanded_width
                    // } else {
                    //     current_width
                    // }
                    current_width
                }
            };
            self.overlap_tracker.rescale_weights();
            self.write_to_disk(Some(local_best.0), true);
            warn!("width: {:.3} -> {:.3} (best: {:.3})", current_width, next_width, best_width);
            self.change_strip_width(next_width);
            current_width = next_width;
            i += 1;
        }

        self.write_to_disk(Some(best_feasible_solution.clone()), true);

        best_feasible_solution
    }

    pub fn separate_layout(&mut self) -> (Solution, OTSnapshot) {
        let mut min_overlap = fsize::INFINITY;
        let mut min_overlap_solution: Option<(Solution, OTSnapshot)> = None;
        let initial_overlap = min_overlap;
        warn!("initial overlap: {:.3}", initial_overlap);

        let mut n_strikes = 0;

        while n_strikes < N_STRIKES {
            if let Some(min_overlap_solution) = min_overlap_solution.as_ref() {
                warn!("rolling back to min overlap");
                self.rollback(&min_overlap_solution.0, &min_overlap_solution.1);
            }
            self.overlap_tracker.rescale_weights();
            let mut n_iter_no_improvement = 0;
            let mut total_movement = 0;
            let mut total_iter = 0;
            let initial_strike_overlap = self.overlap_tracker.get_total_overlap();

            let start = Instant::now();
            while n_iter_no_improvement < N_ITER_NO_IMPROVEMENT {
                let weighted_overlap_before = self.overlap_tracker.get_total_weighted_overlap();
                let overlap_before = self.overlap_tracker.get_total_overlap();
                let n_movements = self.modify_greedy();
                let overlap = self.overlap_tracker.get_total_overlap();
                let weighted_overlap = self.overlap_tracker.get_total_weighted_overlap();
                info!("[i:{}]     w_o-1: {} -> {}, n_mov: {}, abs_o: {} (min: {})", n_iter_no_improvement, FMT.fmt2(weighted_overlap_before), FMT.fmt2(weighted_overlap), n_movements, FMT.fmt2(overlap), FMT.fmt2(min_overlap));
                //assert!(FPA(weighted_overlap) <= FPA(weighted_overlap_before), "weighted overlap increased: {} -> {}", weighted_overlap_before, weighted_overlap);
                if overlap == 0.0 {
                    warn!("[i:{}] (V) w_o-1: {} -> {}, n_mov: {}, abs_o: {} (min: {})", n_iter_no_improvement, FMT.fmt2(weighted_overlap_before), FMT.fmt2(weighted_overlap), n_movements, FMT.fmt2(overlap), FMT.fmt2(min_overlap));
                    warn!("separation successful, returning");
                    let non_overlapping_solution = (self.problem.create_solution(None), self.overlap_tracker.create_snapshot());
                    self.write_to_disk(None, false);
                    return non_overlapping_solution;
                } else if overlap < min_overlap {
                    let sol = self.problem.create_solution(None);

                    if !self.tabu_list.sol_is_tabu(&sol) {
                        warn!("[i:{}] (*) w_o-1: {} -> {}, n_mov: {}, abs_o: {} (min: {})", n_iter_no_improvement, FMT.fmt2(weighted_overlap_before), FMT.fmt2(weighted_overlap), n_movements, FMT.fmt2(overlap), FMT.fmt2(min_overlap));
                        min_overlap = overlap;
                        min_overlap_solution = Some((sol, self.overlap_tracker.create_snapshot()));
                        n_iter_no_improvement = 0;
                        //self.overlap_tracker.rescale_weights();
                        //self.write_to_disk(None, true);
                    } else {
                        warn!("[i: {}] tabu solution encountered", n_iter_no_improvement);
                        self.swap_tabu_item();
                        //self.write_to_disk(None, true);
                        //n_iter_no_improvement += 1;
                    }
                } else {
                    n_iter_no_improvement += 1;
                }

                self.overlap_tracker.increment_weights();
                self.write_to_disk(None, false);
                total_movement += n_movements;
                total_iter += 1;
            }
            self.write_to_disk(None, true);
            if initial_strike_overlap * 0.98 <= min_overlap {
                n_strikes += 1;
            } else {
                n_strikes = 0;
            }
            warn!("{:.1} moves/s, {:.1} iter/s, strike: #{}", total_movement as f64 / start.elapsed().as_secs_f64(), total_iter as f64 / start.elapsed().as_secs_f64(), n_strikes);
        }

        min_overlap_solution.unwrap()
    }

    pub fn modify_greedy(&mut self) -> usize {
        let mut n_total_movements = 0;

        for i in 0..1 {
            let candidates = self.problem.layout.placed_items()
                .keys()
                .filter(|pk| self.overlap_tracker.get_overlap(*pk) > 0.0)
                .collect_vec()
                .tap_mut(|v| v.shuffle(&mut self.rng));

            let mut n_movements = 0;

            for &pk in candidates.iter() {
                let current_overlap = self.overlap_tracker.get_overlap(pk);
                if current_overlap > 0.0 {
                    let item = self.instance.item(self.problem.layout.placed_items()[pk].item_id);

                    let evaluator = OverlappingSampleEvaluator::new(
                        &self.problem.layout,
                        item,
                        Some(pk),
                        &self.overlap_tracker,
                    );

                    let new_placement = search::search_placement(
                        &self.problem.layout,
                        item,
                        Some(pk),
                        evaluator,
                        self.generate_search_config(pk),
                        &mut self.rng,
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

    pub fn modify_global(&mut self) ->  usize {
        let mut n_total_movements = 0;
        let mut candidates = self.problem.layout.placed_items()
            .keys()
            .filter(|pk| self.overlap_tracker.get_overlap(*pk) > 0.0)
            .collect_vec();

        while !candidates.is_empty() {
            let mut movements = candidates.iter()
                .map(|&pk| {
                    let item = self.instance.item(self.problem.layout.placed_items()[pk].item_id);
                    let evaluator = OverlappingSampleEvaluator::new(
                        &self.problem.layout,
                        item,
                        Some(pk),
                        &self.overlap_tracker,
                    );

                    let movement = search::search_placement(
                        &self.problem.layout,
                        item,
                        Some(pk),
                        evaluator,
                        self.generate_search_config(pk),
                        &mut self.rng,
                    );
                    (pk, movement.0, movement.1, self.overlap_tracker.get_weighted_overlap(pk))
                }).collect_vec();

            //dbg!(&movements);

            let best_move = movements.into_iter()
                .max_by_key(|(_, _, eval, current_w_o)| {
                    let w_o_eval = match eval {
                        SampleEval::Invalid => unreachable!(),
                        SampleEval::Valid(_) => 0.0,
                        SampleEval::Colliding { w_overlap, .. } => *w_overlap,
                    };
                    let w_o_improvement = current_w_o - w_o_eval;
                    assert!(w_o_improvement >= -0.0001 * current_w_o, "{:.3} -> {:.3}", current_w_o, w_o_eval);
                    OrderedFloat(w_o_improvement)
                },)
                .unwrap();

            let new_pk = self.move_item(best_move.0, best_move.1, Some(best_move.2));
            candidates.retain(|pk| *pk != best_move.0 && self.overlap_tracker.get_overlap(*pk) > 0.0);

            n_total_movements += 1;
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
        self.overlap_tracker = OverlapTracker::new(&self.problem.layout, RESCALE_WEIGHT_TARGET, WEIGHT_INCREMENT, JUMP_COOLDOWN);
        info!("changed strip width to {}", new_width);
    }

    fn move_item(&mut self, pik: PItemKey, d_transf: DTransformation, eval: Option<SampleEval>) -> PItemKey {
        debug_assert!(overlap_tracker_original::tracker_matches_layout(&self.overlap_tracker, &self.problem.layout));

        let old_overlap = self.overlap_tracker.get_overlap(pik);
        let old_weighted_overlap = self.overlap_tracker.get_weighted_overlap(pik);
        let old_bbox = self.problem.layout.placed_items()[pik].shape.bbox();

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

        assert!(colliding_entities.is_empty() || !matches!(eval, Some(SampleEval::Valid(_))), "colliding entities detected for valid placement");

        let new_pk = {
            let new_p_opt = PlacingOption {
                d_transf,
                ..old_p_opt
            };

            let (_, new_pik) = self.problem.place_item(new_p_opt);
            new_pik
        };

        //info!("moving item {} from {:?} to {:?} ({:?}->{:?})", item.id, old_p_opt.d_transf, new_p_opt.d_transf, pik, new_pik);

        self.overlap_tracker.move_item(&self.problem.layout, pik, new_pk);

        let new_overlap = self.overlap_tracker.get_overlap(new_pk);
        let new_weighted_overlap = self.overlap_tracker.get_weighted_overlap(new_pk);
        let new_bbox = self.problem.layout.placed_items()[new_pk].shape.bbox();

        let jumped = old_bbox.relation_to(&new_bbox) == GeoRelation::Disjoint;
        let item_big_enough = item.shape.surrogate().convex_hull_area > self.tabu_list.ch_area_cutoff;
        if jumped && item_big_enough {
            self.overlap_tracker.set_jumped(new_pk);
        }

        debug!("Moved item {} from from o: {}, wo: {} to o+1: {}, w_o+1: {} (jump: {})", item.id, FMT.fmt2(old_overlap), FMT.fmt2(old_weighted_overlap), FMT.fmt2(new_overlap), FMT.fmt2(new_weighted_overlap), jumped);

        debug_assert!(overlap_tracker_original::tracker_matches_layout(&self.overlap_tracker, &self.problem.layout));

        new_pk
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
                let filename = format!("{}/{}_{:.2}_s.svg", &self.output_folder, self.svg_counter, sol.layout_snapshots[0].bin.bbox().x_max);
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

    pub fn rollback(&mut self, solution: &Solution, ots: &OTSnapshot) {
        self.problem.restore_to_solution(solution);
        self.overlap_tracker.restore(ots, &self.problem.layout);
    }

    pub fn swap_tabu_item(&mut self) {
        warn!("swapping tabu item");
        let layout = &self.problem.layout;
        let (pk1, pi1) = layout.placed_items.iter()
            .filter(|(_, pi)| pi.shape.surrogate().convex_hull_area > self.tabu_list.ch_area_cutoff)
            .choose(&mut self.rng)
            .unwrap();

        let (pk2, pi2) = layout.placed_items.iter()
            .filter(|(_, pi)| pi.item_id != pi1.item_id)
            .filter(|(_, pi)| pi.shape.surrogate().convex_hull_area > self.tabu_list.ch_area_cutoff)
            .choose(&mut self.rng)
            .unwrap();

        let dtransf1 = pi1.d_transf;
        let dtransf2 = pi2.d_transf;

        let new_pk1 = self.move_item(pk1, dtransf2, None);
        let new_pk2 = self.move_item(pk2, dtransf1, None);
        self.overlap_tracker.set_pair_weight(new_pk1, new_pk2, RESCALE_WEIGHT_TARGET * 2.0);
    }

    pub fn generate_search_config(&self, pk: PItemKey) -> SearchConfig {
        let on_jump_cooldown = self.overlap_tracker.is_on_jump_cooldown(pk);
        match on_jump_cooldown {
            false => SearchConfig {
                n_bin_samples: N_UNIFORM_SAMPLES / 2,
                n_focussed_samples: N_UNIFORM_SAMPLES / 2,
                n_coord_descents: N_COORD_DESCENTS,
            },
            true => SearchConfig {
                n_bin_samples: 0,
                n_focussed_samples: N_UNIFORM_SAMPLES,
                n_coord_descents: N_COORD_DESCENTS,
            }
        }
    }
}