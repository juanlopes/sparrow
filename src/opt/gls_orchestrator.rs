use crate::config::{DRAW_OPTIONS, LARGE_ITEM_CH_AREA_CUTOFF_RATIO, N_ITER_NO_IMPROVEMENT, N_STRIKES, N_WORKERS, OUTPUT_DIR, R_SHRINK, STDDEV_SPREAD};
use crate::opt::constr_builder::ConstructiveBuilder;
use crate::opt::gls_worker::GLSWorker;
use crate::overlap::tracker::{OTSnapshot, OverlapTracker};
use crate::sample::eval::SampleEval;
use crate::util::assertions::tracker_matches_layout;
use crate::util::io;
use crate::util::io::layout_to_svg::{layout_to_svg, s_layout_to_svg};
use crate::FMT;
use itertools::Itertools;
use jagua_rs::entities::bin::Bin;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::instances::strip_packing::SPInstance;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::problem_generic::{ProblemGeneric, STRIP_LAYOUT_IDX};
use jagua_rs::entities::problems::strip_packing::{strip_width, SPProblem};
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_enums::GeoRelation;
use jagua_rs::geometry::geo_traits::{Shape, Transformable};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::util::fpa::FPA;
use log::{debug, log, Level};
use ordered_float::OrderedFloat;
use rand::prelude::IteratorRandom;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rand_distr::Distribution;
use rand_distr::Normal;
use rayon::iter::IntoParallelRefMutIterator;
use rayon::iter::ParallelIterator;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct GLSOrchestrator {
    pub instance: SPInstance,
    pub rng: SmallRng,
    pub prob: SPProblem,
    pub ot: OverlapTracker,
    pub workers: Vec<GLSWorker>,
    pub output_folder: String,
    pub svg_counter: usize,
    pub large_area_ch_area_cutoff: fsize,
    pub log_level: log::Level,
}

impl GLSOrchestrator {
    pub fn from_builder(
        mut init_builder: ConstructiveBuilder,
        output_folder: String,
    ) -> Self {
        init_builder.construct();
        let ConstructiveBuilder { instance, prob, mut rng, .. } = init_builder;

        let overlap_tracker = OverlapTracker::new(&prob.layout);
        let large_area_ch_area_cutoff = instance.items().iter()
            .map(|(item, _)| item.shape.surrogate().convex_hull_area)
            .max_by_key(|&x| OrderedFloat(x))
            .unwrap() * LARGE_ITEM_CH_AREA_CUTOFF_RATIO;
        let workers = (0..N_WORKERS)
            .map(|_| GLSWorker {
                instance: instance.clone(),
                prob: prob.clone(),
                ot: overlap_tracker.clone(),
                rng: SmallRng::seed_from_u64(rng.random()),
                large_area_ch_area_cutoff,
            })
            .collect();

        Self {
            prob,
            instance,
            rng,
            ot: overlap_tracker,
            workers,
            svg_counter: 0,
            output_folder,
            large_area_ch_area_cutoff,
            log_level: log::Level::Info,
        }
    }

    pub fn solve(&mut self, time_out: Duration) -> Vec<Solution> {
        let mut current_width = self.prob.occupied_width();
        let mut best_width = current_width;

        let mut feasible_solutions = vec![self.prob.create_solution(None)];

        self.write_to_disk(None, "init", true);
        log!(self.log_level,"[GLS] starting optimization with initial width: {:.3} ({:.3}%)",current_width,self.prob.usage() * 100.0);

        let end_time = Instant::now() + time_out;
        let mut solution_pool: Vec<(Solution, fsize)> = vec![];

        while Instant::now() < end_time {
            let local_best = self.separate_layout(Some(end_time));
            let total_overlap = local_best.1.total_overlap;

            if total_overlap == 0.0 {
                //layout is successfully separated
                if current_width < best_width {
                    log!(self.log_level,"[GLS] new best at width: {:.3} ({:.3}%)",current_width,self.prob.usage() * 100.0);
                    best_width = current_width;
                    feasible_solutions.push(local_best.0.clone());
                    self.write_to_disk(Some(local_best.0.clone()), "f", true);
                }
                let next_width = current_width * (1.0 - R_SHRINK);
                log!(self.log_level,"[GLS] shrinking width by {}%: {:.3} -> {:.3}", R_SHRINK * 100.0, current_width, next_width);
                self.change_strip_width(next_width, None);
                current_width = next_width;
                solution_pool.clear();
            } else {
                log!(self.log_level,"[GLS] layout separation unsuccessful, best overlap: {}", FMT.fmt2(total_overlap));
                self.write_to_disk(Some(local_best.0.clone()), "o", true);

                //layout was not successfully separated, add to local bests
                match solution_pool.binary_search_by(|(_, o)| o.partial_cmp(&total_overlap).unwrap()) {
                    Ok(idx) | Err(idx) => solution_pool.insert(idx, (local_best.0.clone(), total_overlap)),
                }

                //make sure it is sorted correctly
                assert!(solution_pool.is_sorted_by(|(_, o1), (_, o2)| o1 <= o2));

                //restore to a random solution from the tabu list, better solutions have more chance to be selected
                let selected_sol = {
                    let distr = Normal::new(0.0 as fsize, solution_pool.len() as fsize / STDDEV_SPREAD).unwrap();
                    let selected_idx = (distr.sample(&mut self.rng).abs().floor() as usize).min(solution_pool.len() - 1);
                    let selected = solution_pool.get(selected_idx).unwrap();
                    log!(self.log_level,"[GLS] selected starting solution {}/{} from solution pool (o: {})", selected_idx, solution_pool.len(), FMT.fmt2(selected.1));
                    selected.0.clone()
                };

                self.rollback(&selected_sol, None);
                //swap two large items
                self.swap_large_pair_of_items();
            }
        }

        log!(self.log_level,"[GLS] time limit reached, best solution found: {:.3} ({:.3}%)",best_width,feasible_solutions.last().unwrap().usage * 100.0);

        feasible_solutions
    }

    pub fn separate_layout(&mut self, time_out: Option<Instant>) -> (Solution, OTSnapshot) {
        let mut min_overlap_sol: (Solution, OTSnapshot) = (self.prob.create_solution(None), self.ot.create_snapshot());
        let mut min_overlap = self.ot.get_total_overlap();
        log!(self.log_level,"[SEP] separating at width: {:.3} and overlap: {} ", self.prob.strip_width(), FMT.fmt2(min_overlap));

        let mut n_strikes = 0;
        let mut n_iter = 0;
        let mut n_items_moved = 0;
        let start = Instant::now();

        while n_strikes < N_STRIKES && time_out.map_or(true, |t| Instant::now() < t) {
            let mut n_iter_no_improvement = 0;

            let initial_strike_overlap = self.ot.get_total_overlap();
            log!(self.log_level,"[SEP] [s:{n_strikes}] init_o: {}",FMT.fmt2(initial_strike_overlap));

            while n_iter_no_improvement < N_ITER_NO_IMPROVEMENT {
                let (overlap_before, w_overlap_before) = (
                    self.ot.get_total_overlap(),
                    self.ot.get_total_weighted_overlap(),
                );
                let n_moves = self.modify();
                let (overlap, w_overlap) = (
                    self.ot.get_total_overlap(),
                    self.ot.get_total_weighted_overlap(),
                );

                debug!("[SEP] [s:{n_strikes},i:{n_iter}] ( ) o: {} -> {}, w_o: {} -> {}, #mv: {}, (min o: {})",FMT.fmt2(overlap_before),FMT.fmt2(overlap),FMT.fmt2(w_overlap_before),FMT.fmt2(w_overlap),n_moves,FMT.fmt2(min_overlap));
                debug_assert!(FPA(w_overlap) <= FPA(w_overlap_before), "weighted overlap increased: {} -> {}", FMT.fmt2(w_overlap_before), FMT.fmt2(w_overlap));

                if overlap == 0.0 {
                    //layout is successfully separated
                    log!(self.log_level,"[SEP] [s:{n_strikes},i:{n_iter}] (S)  min_o: {}",FMT.fmt2(overlap));
                    return (self.prob.create_solution(None), self.ot.create_snapshot());
                } else if overlap < min_overlap {
                    //layout is not separated, but absolute overlap is better than before
                    let sol = self.prob.create_solution(None);
                    min_overlap_sol = (sol, self.ot.create_snapshot());
                    min_overlap = overlap;
                    log!(self.log_level,"[SEP] [s:{n_strikes},i:{n_iter}] (*) min_o: {}",FMT.fmt2(overlap));
                    n_iter_no_improvement = 0;
                } else {
                    n_iter_no_improvement += 1;
                }

                self.ot.increment_weights();
                n_items_moved += n_moves;
                n_iter += 1;
            }
            self.write_to_disk(None, "strike", false);

            if initial_strike_overlap * 0.98 <= min_overlap {
                n_strikes += 1;
            } else {
                n_strikes = 0;
            }
            self.rollback(&min_overlap_sol.0, Some(&min_overlap_sol.1));
        }
        log!(self.log_level,"[SEP] strike limit reached ({}), moves/s: {}, iter/s: {}, time: {}ms",n_strikes,(n_items_moved as f64 / start.elapsed().as_secs_f64()) as usize,(n_iter as f64 / start.elapsed().as_secs_f64()) as usize,start.elapsed().as_millis());

        (min_overlap_sol.0, min_overlap_sol.1)
    }

    pub fn modify(&mut self) -> usize {
        let master_sol = self.prob.create_solution(None);

        let n_movements = self.workers.par_iter_mut()
            .map(|worker| {
                // Sync the workers with the master
                worker.load(&master_sol, &self.ot);
                // Let them modify
                let n_moves = worker.separate();
                n_moves
            })
            .sum();

        debug!("[MOD] optimizers w_o's: {:?}",self.workers.iter().map(|opt| opt.ot.get_total_weighted_overlap()).collect_vec());

        // Check which worker has the lowest total weighted overlap
        let best_opt = self.workers.iter_mut()
            .min_by_key(|opt| OrderedFloat(opt.ot.get_total_weighted_overlap()))
            .map(|opt| (opt.prob.create_solution(None), &opt.ot))
            .unwrap();

        // Sync the master with the best optimizer
        self.prob.restore_to_solution(&best_opt.0);
        self.ot = best_opt.1.clone();

        n_movements
    }

    pub fn rollback(&mut self, sol: &Solution, ots: Option<&OTSnapshot>) {
        assert_eq!(strip_width(sol), self.prob.strip_width());
        self.prob.restore_to_solution(sol);

        match ots {
            Some(ots) => {
                //if a snapshot of the overlap tracker was provided, restore it
                self.ot.restore_but_keep_weights(ots, &self.prob.layout);
            }
            None => {
                //otherwise, rebuild it
                self.ot = OverlapTracker::new(&self.prob.layout);
            }
        }
    }

    pub fn swap_large_pair_of_items(&mut self) {
        let layout = &self.prob.layout;
        let (pk1, pi1) = layout.placed_items.iter()
            .filter(|(_, pi)| pi.shape.surrogate().convex_hull_area > self.large_area_ch_area_cutoff)
            .choose(&mut self.rng)
            .unwrap();

        let (pk2, pi2) = layout.placed_items.iter()
            .filter(|(_, pi)| pi.item_id != pi1.item_id)
            .filter(|(_, pi)| pi.shape.surrogate().convex_hull_area > self.large_area_ch_area_cutoff)
            .choose(&mut self.rng)
            .unwrap_or(layout.placed_items.iter().choose(&mut self.rng).unwrap());

        let dt1 = pi1.d_transf;
        let dt2 = pi2.d_transf;

        log!(self.log_level,"[GLS] swapped two large items (id: {} <-> {})", pi1.item_id, pi2.item_id);

        self.move_item(pk1, dt2, None);
        self.move_item(pk2, dt1, None);
    }

    fn move_item(&mut self, pik: PItemKey, dt: DTransformation, eval: Option<SampleEval>) -> PItemKey {
        debug_assert!(tracker_matches_layout(
            &self.ot,
            &self.prob.layout
        ));

        let old_overlap = self.ot.get_overlap(pik);
        let old_weighted_overlap = self.ot.get_weighted_overlap(pik);
        let old_bbox = self.prob.layout.placed_items()[pik].shape.bbox();

        //Remove the item from the problem
        let old_p_opt = self.prob.remove_item(STRIP_LAYOUT_IDX, pik, true);
        let item = self.instance.item(old_p_opt.item_id);

        //Compute the colliding entities after the move
        let colliding_entities = {
            let shape = item.shape.transform_clone(&dt.into());
            self.prob
                .layout
                .cde()
                .collect_poly_collisions(&shape, &[])
        };

        assert!(
            colliding_entities.is_empty() || !matches!(eval, Some(SampleEval::Valid(_))),
            "colliding entities detected for valid placement"
        );

        let new_pk = {
            let new_p_opt = PlacingOption {
                d_transf: dt,
                ..old_p_opt
            };

            let (_, new_pik) = self.prob.place_item(new_p_opt);
            new_pik
        };

        self.ot
            .register_item_move(&self.prob.layout, pik, new_pk);

        let new_overlap = self.ot.get_overlap(new_pk);
        let new_weighted_overlap = self.ot.get_weighted_overlap(new_pk);
        let new_bbox = self.prob.layout.placed_items()[new_pk].shape.bbox();

        let jumped = old_bbox.relation_to(&new_bbox) == GeoRelation::Disjoint;
        let item_big_enough =
            item.shape.surrogate().convex_hull_area > self.large_area_ch_area_cutoff;
        if jumped && item_big_enough {
            self.ot.register_jump(new_pk);
        }

        debug!("[MV] moved item {} from from o: {}, wo: {} to o+1: {}, w_o+1: {} (jump: {})",item.id,FMT.fmt2(old_overlap),FMT.fmt2(old_weighted_overlap),FMT.fmt2(new_overlap),FMT.fmt2(new_weighted_overlap),jumped);

        debug_assert!(tracker_matches_layout(&self.ot, &self.prob.layout));

        new_pk
    }

    pub fn change_strip_width(&mut self, new_width: fsize, split_position: Option<fsize>) {
        let current_width = self.prob.strip_width();
        let delta = new_width - current_width;
        //shift all items right of the center of the strip

        let split_position = split_position.unwrap_or(current_width / 2.0);

        let shift_transf = DTransformation::new(0.0, (delta + FPA::tolerance(), 0.0));
        let items_to_shift = self.prob.layout.placed_items().iter()
            .filter(|(_, pi)| pi.shape.centroid().0 > split_position)
            .map(|(k, pi)| (k, pi.d_transf))
            .collect_vec();

        for (pik, dtransf) in items_to_shift {
            let new_transf = dtransf.compose().translate(shift_transf.translation());
            self.move_item(pik, new_transf.decompose(), None);
        }

        let new_bin = Bin::from_strip(
            AARectangle::new(0.0, 0.0, new_width, self.prob.strip_height()),
            self.prob.layout.bin.base_cde.config().clone(),
        );
        self.prob.layout.change_bin(new_bin);
        self.ot = OverlapTracker::new(&self.prob.layout);

        self.workers.iter_mut().for_each(|opt| {
            *opt = GLSWorker {
                instance: self.instance.clone(),
                prob: self.prob.clone(),
                ot: self.ot.clone(),
                rng: SmallRng::seed_from_u64(self.rng.random()),
                large_area_ch_area_cutoff: self.large_area_ch_area_cutoff,
            };
        });
        debug!("[GLS] changed strip width to {:.3}", new_width);
    }

    pub fn write_to_disk(&mut self, solution: Option<Solution>, suffix: &str, force: bool) {
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
                let file_name = format!("{}/{}_{:.2}_{suffix}.svg", &self.output_folder, self.svg_counter, strip_width(&sol));
                let file_path = Path::new(&file_name);
                let title = file_path.file_stem().unwrap().to_str().unwrap();
                let svg = s_layout_to_svg(&sol.layout_snapshots[0], &self.instance, DRAW_OPTIONS, title);
                io::write_svg(&svg, file_path, self.log_level);
                io::write_svg(&svg, Path::new(&format!("{}/live_solution.svg", OUTPUT_DIR)), Level::Trace);
            }
            None => {
                let file_name = format!("{}/{}_{:.2}_{suffix}.svg", &self.output_folder, self.svg_counter, self.prob.strip_width());
                let file_path = Path::new(&file_name);
                let title = file_path.file_stem().unwrap().to_str().unwrap();
                let svg = layout_to_svg(&self.prob.layout, &self.instance, DRAW_OPTIONS, title);
                io::write_svg(&svg, file_path, self.log_level);
                io::write_svg(&svg, Path::new(&format!("{}/live_solution.svg", OUTPUT_DIR)), Level::Trace);
            }
        }

        self.svg_counter += 1;
    }
}
