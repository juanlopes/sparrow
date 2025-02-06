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
use rand::distributions::{Uniform, WeightedError, WeightedIndex};
use rand::prelude::{Distribution, IteratorRandom, SliceRandom, SmallRng};
use std::char::decode_utf16;
use std::cmp::Reverse;
use std::collections::VecDeque;
use std::iter;
use std::ops::Range;
use std::path::Path;
use std::time::{Duration, Instant};
use jagua_rs::geometry::geo_enums::GeoRelation;
use num_traits::real::Real;
use rand::{Rng, SeedableRng};
use rayon::iter::{split, ParallelIterator};
use rayon::iter::IntoParallelRefMutIterator;
use tap::Tap;
use crate::opt::gls_optimizer::GLSOptimizer;

const N_ITER_NO_IMPROVEMENT: usize = 50;

const N_STRIKES: usize = 5;
const R_SHRINK: fsize = 0.005;
//const R_EXPAND: fsize = 0.003;

const N_UNIFORM_SAMPLES: usize = 100;
const N_COORD_DESCENTS: usize = 2;
const RESCALE_WEIGHT_TARGET: fsize = 2.0;

const WEIGHT_INCREMENT: fsize = 1.2;
const TABU_SIZE: usize = 10000;
const JUMP_COOLDOWN: usize = 2;

const N_THREADS: usize = 4;

const N_MOVEMENTS: usize = usize::MAX;

pub struct GLSOrchestrator {
    pub instance: SPInstance,
    pub rng: SmallRng,
    pub master_prob: SPProblem,
    pub master_ot: OverlapTracker,
    pub optimizers: Vec<GLSOptimizer>,
    pub output_folder: String,
    pub svg_counter: usize,
    pub tabu_list: TabuList,
}

impl GLSOrchestrator {
    pub fn new(
        problem: SPProblem,
        instance: SPInstance,
        mut rng: SmallRng,
        output_folder: String,
    ) -> Self {
        let overlap_tracker = OverlapTracker::new(&problem.layout, RESCALE_WEIGHT_TARGET, WEIGHT_INCREMENT, JUMP_COOLDOWN);
        let tabu_list = TabuList::new(TABU_SIZE, &instance);
        let optimizers = (0..N_THREADS)
            .map(|i| {
                let output_folder = format!("{}/opt_{}", output_folder, i);
                GLSOptimizer::new(problem.clone(), instance.clone(), SmallRng::seed_from_u64(rng.gen()), output_folder)
            })
            .collect();
        Self {
            master_prob: problem.clone(),
            instance,
            rng,
            master_ot: overlap_tracker.clone(),
            optimizers,
            svg_counter: 0,
            output_folder,
            tabu_list,
        }
    }

    pub fn solve(&mut self, time_out: Duration) -> Solution {
        //self.change_strip_width(5750.0);
        let mut current_width = self.master_prob.occupied_width();
        let (mut best_feasible_solution, mut best_width) = (self.master_prob.create_solution(None), current_width);


        self.write_to_disk(None, true);

        let start = Instant::now();
        let mut i = 0;

        while start.elapsed() < time_out {
            let local_best = self.separate_layout();
            let total_overlap = self.master_ot.get_total_overlap();
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
                    //self.rollback(&local_best.0, &local_best.1);
                    self.tabu_list.push(local_best.0.clone(), total_overlap);
                    warn!("adding local best to tabu list");
                    {
                        const WEIGHTED_INDEX: [fsize; 5] = [256.0, 128.0, 64.0, 32.0, 16.0];

                        let n_th_best_solution = WeightedIndex::new(WEIGHTED_INDEX).unwrap().sample(&mut self.rng);

                        let sorted_tabu_sols = self.tabu_list.list.iter()
                            .filter(|(sol, eval)| sol.layout_snapshots[0].bin.bbox().width() == current_width)
                            .sorted_by_key(|(sol, eval)| OrderedFloat(*eval))
                            .collect_vec();

                        let n_th_best_solution = n_th_best_solution.min(sorted_tabu_sols.len() - 1);
                        let selected = sorted_tabu_sols[n_th_best_solution].clone();

                        warn!("Rolling back to {}/{} best solution from the tabu list (o: {:.3})", n_th_best_solution, sorted_tabu_sols.len(), selected.1);

                        self.rollback(&selected.0, None);
                    }
                    current_width
                }
            };
            self.master_ot.rescale_weights();
            self.write_to_disk(Some(local_best.0), true);
            warn!("width: {:.3} -> {:.3} (best: {:.3})", current_width, next_width, best_width);
            if next_width != current_width {
                self.change_strip_width(next_width, None);
            }
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
                self.rollback(&min_overlap_solution.0, Some(&min_overlap_solution.1));
            }
            self.master_ot.rescale_weights();
            let mut n_iter_no_improvement = 0;
            let mut total_movement = 0;
            let mut total_iter = 0;
            let initial_strike_overlap = self.master_ot.get_total_overlap();

            let start = Instant::now();
            while n_iter_no_improvement < N_ITER_NO_IMPROVEMENT {
                let weighted_overlap_before = self.master_ot.get_total_weighted_overlap();
                let overlap_before = self.master_ot.get_total_overlap();
                let n_movements = self.modify();
                let overlap = self.master_ot.get_total_overlap();
                let weighted_overlap = self.master_ot.get_total_weighted_overlap();
                info!("[i:{}]     w_o-1: {} -> {}, n_mov: {}, abs_o: {} (min: {})", n_iter_no_improvement, FMT.fmt2(weighted_overlap_before), FMT.fmt2(weighted_overlap), n_movements, FMT.fmt2(overlap), FMT.fmt2(min_overlap));
                //assert!(FPA(weighted_overlap) <= FPA(weighted_overlap_before), "weighted overlap increased: {} -> {}", weighted_overlap_before, weighted_overlap);
                if overlap == 0.0 {
                    warn!("[i:{}] (V) w_o-1: {} -> {}, n_mov: {}, abs_o: {} (min: {})", n_iter_no_improvement, FMT.fmt2(weighted_overlap_before), FMT.fmt2(weighted_overlap), n_movements, FMT.fmt2(overlap), FMT.fmt2(min_overlap));
                    warn!("separation successful, returning");
                    let non_overlapping_solution = (self.master_prob.create_solution(None), self.master_ot.create_snapshot());
                    self.write_to_disk(None, false);
                    return non_overlapping_solution;
                } else if overlap < min_overlap {
                    let sol = self.master_prob.create_solution(None);

                    if !self.tabu_list.sol_is_tabu(&sol) {
                        warn!("[i:{}] (*) w_o-1: {} -> {}, n_mov: {}, abs_o: {} (min: {})", n_iter_no_improvement, FMT.fmt2(weighted_overlap_before), FMT.fmt2(weighted_overlap), n_movements, FMT.fmt2(overlap), FMT.fmt2(min_overlap));
                        min_overlap = overlap;
                        min_overlap_solution = Some((sol, self.master_ot.create_snapshot()));
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

                self.master_ot.increment_weights();
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

    pub fn modify(&mut self) -> usize {

        let master_sol = self.master_prob.create_solution(None);

        self.optimizers.par_iter_mut()
            .for_each(|opt| {
                // Sync the workers
                opt.load(&master_sol, &self.master_ot);
                // Let them modify
                opt.modify_greedy(Some(N_MOVEMENTS));
            });

        info!("{:?}", self.optimizers.iter().map(|opt| opt.overlap_tracker.get_total_weighted_overlap()).collect_vec());

        // Save the best one
        let best_opt = self.optimizers.iter_mut()
            .min_by_key(|opt|{
                let w_o = opt.overlap_tracker.get_total_weighted_overlap();
                OrderedFloat(w_o)
            })
            .map(|opt| (opt.problem.create_solution(None), &opt.overlap_tracker))
            .unwrap();

        self.master_prob.restore_to_solution(&best_opt.0);
        self.master_ot = best_opt.1.clone();

        0
    }

    pub fn rollback(&mut self, solution: &Solution, ots: Option<&OTSnapshot>) {
        self.master_prob.restore_to_solution(solution);

        if let Some(ots) = ots {
            self.master_ot.restore(ots, &self.master_prob.layout);
        }
        else {
            self.master_ot = OverlapTracker::new(&self.master_prob.layout, RESCALE_WEIGHT_TARGET, WEIGHT_INCREMENT, JUMP_COOLDOWN);
        }
    }

    pub fn swap_tabu_item(&mut self) {
        warn!("swapping tabu item");
        let layout = &self.master_prob.layout;
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
        self.master_ot.set_pair_weight(new_pk1, new_pk2, RESCALE_WEIGHT_TARGET * 2.0);
    }

    fn move_item(&mut self, pik: PItemKey, d_transf: DTransformation, eval: Option<SampleEval>) -> PItemKey {
        debug_assert!(overlap_tracker_original::tracker_matches_layout(&self.master_ot, &self.master_prob.layout));

        let old_overlap = self.master_ot.get_overlap(pik);
        let old_weighted_overlap = self.master_ot.get_weighted_overlap(pik);
        let old_bbox = self.master_prob.layout.placed_items()[pik].shape.bbox();

        //Remove the item from the problem
        let old_p_opt = self.master_prob.remove_item(STRIP_LAYOUT_IDX, pik, true);
        let item = self.instance.item(old_p_opt.item_id);

        //Compute the colliding entities after the move
        let colliding_entities = {
            let shape = item.shape.clone().as_ref().clone()
                .tap_mut(|s| { s.transform(&d_transf.compose()); });

            let mut colliding_entities = vec![];
            self.master_prob.layout.cde().collect_poly_collisions(&shape, &[], &mut colliding_entities);
            colliding_entities
        };

        assert!(colliding_entities.is_empty() || !matches!(eval, Some(SampleEval::Valid(_))), "colliding entities detected for valid placement");

        let new_pk = {
            let new_p_opt = PlacingOption {
                d_transf,
                ..old_p_opt
            };

            let (_, new_pik) = self.master_prob.place_item(new_p_opt);
            new_pik
        };

        //info!("moving item {} from {:?} to {:?} ({:?}->{:?})", item.id, old_p_opt.d_transf, new_p_opt.d_transf, pik, new_pik);

        self.master_ot.move_item(&self.master_prob.layout, pik, new_pk);

        let new_overlap = self.master_ot.get_overlap(new_pk);
        let new_weighted_overlap = self.master_ot.get_weighted_overlap(new_pk);
        let new_bbox = self.master_prob.layout.placed_items()[new_pk].shape.bbox();

        let jumped = old_bbox.relation_to(&new_bbox) == GeoRelation::Disjoint;
        let item_big_enough = item.shape.surrogate().convex_hull_area > self.tabu_list.ch_area_cutoff;
        if jumped && item_big_enough {
            self.master_ot.set_jumped(new_pk);
        }

        debug!("Moved item {} from from o: {}, wo: {} to o+1: {}, w_o+1: {} (jump: {})", item.id, FMT.fmt2(old_overlap), FMT.fmt2(old_weighted_overlap), FMT.fmt2(new_overlap), FMT.fmt2(new_weighted_overlap), jumped);

        debug_assert!(overlap_tracker_original::tracker_matches_layout(&self.master_ot, &self.master_prob.layout));

        new_pk
    }

    pub fn change_strip_width(&mut self, new_width: fsize, split_position: Option<fsize>) {
        let current_width = self.master_prob.strip_width();
        let delta = new_width - current_width;
        //shift all items right of the center of the strip

        let split_position = split_position.unwrap_or(current_width / 2.0);

        let shift_transf = DTransformation::new(0.0, (delta + FPA::tolerance(), 0.0));
        let items_to_shift = self.master_prob.layout.placed_items().iter()
            .filter(|(_, pi)| pi.shape.centroid().0 > split_position)
            .map(|(k, pi)| (k, pi.d_transf))
            .collect_vec();

        for (pik, dtransf) in items_to_shift {
            let new_transf = dtransf.compose().translate(shift_transf.translation());
            self.move_item(pik, new_transf.decompose(), None);
        }

        let new_bin = Bin::from_strip(
            AARectangle::new(0.0, 0.0, new_width, self.master_prob.strip_height()),
            self.master_prob.layout.bin.base_cde.config().clone(),
        );
        self.master_prob.layout.change_bin(new_bin);
        self.master_ot = OverlapTracker::new(&self.master_prob.layout, RESCALE_WEIGHT_TARGET, WEIGHT_INCREMENT, JUMP_COOLDOWN);

        self.optimizers.iter_mut().for_each(|opt| {
            opt.change_strip_width(new_width);
        });

        info!("changed strip width to {}", new_width);
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
                let filename = format!("{}/{}_{:.2}.svg", &self.output_folder, self.svg_counter, self.master_prob.layout.bin.bbox().x_max);
                io::write_svg(
                    &layout_to_svg(&self.master_prob.layout, &self.instance, SvgDrawOptions::default()),
                    Path::new(&filename),
                );
                warn!("wrote layout to disk: {}", filename);
            }
        }

        self.svg_counter += 1;
    }

}