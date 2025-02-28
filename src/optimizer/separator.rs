use crate::config::{DRAW_OPTIONS, EXPORT_LIVE_SVG, OUTPUT_DIR};
use crate::optimizer::separator_worker::SeparatorWorker;
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
use rayon::iter::IntoParallelRefMutIterator;
use rayon::iter::ParallelIterator;
use std::path::Path;
use std::time::Instant;

pub struct SeparatorConfig {
    pub iter_no_imprv_limit: usize,
    pub strike_limit: usize,
    pub n_workers: usize,
    pub log_level: Level,
    pub jump_cooldown: usize,
    pub large_area_ch_area_cutoff_ratio: fsize,
}

pub struct Separator {
    pub instance: SPInstance,
    pub rng: SmallRng,
    pub prob: SPProblem,
    pub ot: OverlapTracker,
    pub workers: Vec<SeparatorWorker>,
    pub svg_counter: usize,
    pub output_svg_folder: String,
    pub config: SeparatorConfig,
    pub large_area_ch_area_cutoff: fsize,
}

impl Separator {
    pub fn new(instance: SPInstance, prob: SPProblem, mut rng: SmallRng, output_svg_folder: String, svg_counter: usize, config: SeparatorConfig) -> Self {
        //use the builder to create an initial placement into the problem

        let overlap_tracker = OverlapTracker::new(&prob.layout, config.jump_cooldown);
        let large_area_ch_area_cutoff = instance.items().iter()
            .map(|(item, _)| item.shape.surrogate().convex_hull_area)
            .max_by_key(|&x| OrderedFloat(x))
            .unwrap() * config.large_area_ch_area_cutoff_ratio;
        let workers = (0..config.n_workers).map(|_|
            SeparatorWorker {
                instance: instance.clone(),
                prob: prob.clone(),
                ot: overlap_tracker.clone(),
                rng: SmallRng::seed_from_u64(rng.random()),
                large_area_ch_area_cutoff,
            }).collect();

        Self {
            prob,
            instance,
            rng,
            ot: overlap_tracker,
            workers,
            svg_counter,
            output_svg_folder,
            config,
            large_area_ch_area_cutoff,
        }
    }

    pub fn separate_layout(&mut self, time_out: Option<Instant>) -> (Solution, OTSnapshot) {
        let mut min_overlap_sol: (Solution, OTSnapshot) = (self.prob.create_solution(None), self.ot.create_snapshot());
        let mut min_overlap = self.ot.get_total_overlap();
        log!(self.config.log_level,"[SEP] separating at width: {:.3} and overlap: {} ", self.prob.strip_width(), FMT.fmt2(min_overlap));

        let mut n_strikes = 0;
        let mut n_iter = 0;
        let mut n_items_moved = 0;
        let start = Instant::now();

        while n_strikes < self.config.strike_limit && time_out.map_or(true, |t| Instant::now() < t) {
            let mut n_iter_no_improvement = 0;

            let initial_strike_overlap = self.ot.get_total_overlap();
            debug!("[SEP] [s:{n_strikes},i:{n_iter}]     init_o: {}",FMT.fmt2(initial_strike_overlap));

            while n_iter_no_improvement < self.config.iter_no_imprv_limit {
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
                    log!(self.config.log_level,"[SEP] [s:{n_strikes},i:{n_iter}] (S)  min_o: {}",FMT.fmt2(overlap));
                    return (self.prob.create_solution(None), self.ot.create_snapshot());
                } else if overlap < min_overlap {
                    //layout is not separated, but absolute overlap is better than before
                    let sol = self.prob.create_solution(None);

                    log!(self.config.log_level,"[SEP] [s:{n_strikes},i:{n_iter}] (*) min_o: {}",FMT.fmt2(overlap));
                    self.export_svg(None, "i", true);
                    if overlap < min_overlap * 0.98 {
                        //only reset the iter_no_improvement counter if the overlap improved significantly
                        n_iter_no_improvement = 0;
                    }
                    min_overlap_sol = (sol, self.ot.create_snapshot());
                    min_overlap = overlap;
                } else {
                    n_iter_no_improvement += 1;
                }

                self.ot.increment_weights();
                n_items_moved += n_moves;
                n_iter += 1;
            }

            if initial_strike_overlap * 0.98 <= min_overlap {
                n_strikes += 1;
            } else {
                n_strikes = 0;
            }
            self.rollback(&min_overlap_sol.0, Some(&min_overlap_sol.1));
        }
        log!(self.config.log_level,"[SEP] strike limit reached ({}), moves/s: {}, iter/s: {}, time: {}ms",n_strikes,(n_items_moved as f64 / start.elapsed().as_secs_f64()) as usize,(n_iter as f64 / start.elapsed().as_secs_f64()) as usize,start.elapsed().as_millis());

        (min_overlap_sol.0, min_overlap_sol.1)
    }

    fn modify(&mut self) -> usize {
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
                self.ot = OverlapTracker::new(&self.prob.layout, self.config.jump_cooldown);
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

        log!(self.config.log_level,"[GLS] swapped two large items (ids: {} <-> {})", pi1.item_id, pi2.item_id);

        self.move_item(pk1, dt2, None);
        self.move_item(pk2, dt1, None);
    }

    fn move_item(&mut self, pk: PItemKey, d_transf: DTransformation, eval: Option<SampleEval>) -> PItemKey {
        debug_assert!(tracker_matches_layout(&self.ot, &self.prob.layout));

        let item_id = self.prob.layout.placed_items()[pk].item_id;

        let old_overlap = self.ot.get_overlap(pk);
        let old_weighted_overlap = self.ot.get_weighted_overlap(pk);
        let old_bbox = self.prob.layout.placed_items()[pk].shape.bbox();

        //Remove the item from the problem
        self.prob.remove_item(STRIP_LAYOUT_IDX, pk, true);

        //Place the item again but with a new transformation
        let (_, new_pk) = self.prob.place_item(PlacingOption {
            d_transf,
            layout_idx: STRIP_LAYOUT_IDX,
            item_id,
        });

        self.ot.register_item_move(&self.prob.layout, pk, new_pk);

        let new_overlap = self.ot.get_overlap(new_pk);
        let new_weighted_overlap = self.ot.get_weighted_overlap(new_pk);
        let new_bbox = self.prob.layout.placed_items()[new_pk].shape.bbox();

        let jumped = {
            let disjoint_bbox = old_bbox.relation_to(&new_bbox) == GeoRelation::Disjoint;
            let item_ch_area = self.instance.item(item_id).shape.surrogate().convex_hull_area;
            let big_enough = item_ch_area > self.large_area_ch_area_cutoff;
            disjoint_bbox && big_enough
        };

        if jumped {
            self.ot.register_jump(new_pk);
        }

        debug!("[MV] moved item {} from from o: {}, wo: {} to o+1: {}, w_o+1: {} (jump: {})",item_id,FMT.fmt2(old_overlap),FMT.fmt2(old_weighted_overlap),FMT.fmt2(new_overlap),FMT.fmt2(new_weighted_overlap),jumped);

        debug_assert!(tracker_matches_layout(&self.ot, &self.prob.layout));

        new_pk
    }

    pub fn change_strip_width(&mut self, new_width: fsize, split_position: Option<fsize>) {
        //if no split position is provided, use the center of the strip
        let split_position = split_position.unwrap_or(self.prob.strip_width() / 2.0);
        let delta = new_width - self.prob.strip_width();

        //shift all items right of the split position
        let items_to_shift = self.prob.layout.placed_items().iter()
            .filter(|(_, pi)| pi.shape.centroid().0 > split_position)
            .map(|(k, pi)| (k, pi.d_transf))
            .collect_vec();

        for (pik, dtransf) in items_to_shift {
            let existing_transf = dtransf.compose();
            let new_transf = existing_transf.translate((delta, 0.0));
            self.move_item(pik, new_transf.decompose(), None);
        }

        //swap the bin to one with the new width
        let new_bin = Bin::from_strip(
            AARectangle::new(0.0, 0.0, new_width, self.prob.strip_height()),
            self.prob.layout.bin.base_cde.config().clone(),
        );
        self.prob.layout.change_bin(new_bin);

        //rebuild the overlap tracker
        self.ot = OverlapTracker::new(&self.prob.layout, self.config.jump_cooldown);

        //rebuild the workers
        self.workers.iter_mut().for_each(|opt| {
            *opt = SeparatorWorker {
                instance: self.instance.clone(),
                prob: self.prob.clone(),
                ot: self.ot.clone(),
                rng: SmallRng::seed_from_u64(self.rng.random()),
                large_area_ch_area_cutoff: self.large_area_ch_area_cutoff,
            };
        });
        debug!("[GLS] changed strip width to {:.3}", new_width);
    }

    pub fn export_svg(&mut self, solution: Option<Solution>, suffix: &str, only_live: bool) {
        if self.svg_counter == 0 {
            std::fs::create_dir_all(&self.output_svg_folder).unwrap();
            //remove all svg files from the directory. ONLY SVG FILES
            for file in std::fs::read_dir(&self.output_svg_folder).unwrap().flatten() {
                if file.path().extension().unwrap_or_default() == "svg" {
                    std::fs::remove_file(file.path()).unwrap();
                }
            }
        }

        let file_name = format!("{}_{:.3}_{suffix}", self.svg_counter, self.prob.strip_width());
        let svg = match solution {
            Some(sol) => s_layout_to_svg(&sol.layout_snapshots[0], &self.instance, DRAW_OPTIONS, file_name.as_str()),
            None => layout_to_svg(&self.prob.layout, &self.instance, DRAW_OPTIONS, file_name.as_str()),
        };

        if EXPORT_LIVE_SVG {
            io::write_svg(&svg, Path::new(&*format!("{}/.live_solution.svg", OUTPUT_DIR)), Level::Trace);
        }

        if !only_live {
            let file_path = &*format!("{}/{}.svg", &self.output_svg_folder, file_name);
            io::write_svg(&svg, Path::new(file_path), self.config.log_level);
            self.svg_counter += 1;
        }
    }
}
