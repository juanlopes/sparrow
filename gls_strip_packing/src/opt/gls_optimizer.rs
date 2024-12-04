use crate::io::layout_to_svg::layout_to_svg;
use crate::overlap::overlap_tracker;
use crate::overlap::overlap_tracker::OverlapTracker;
use crate::sampl::search::{search_placement, SearchConfig};
use crate::{io, DRAW_OPTIONS, OUTPUT_DIR, SVG_OUTPUT_DIR};
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
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::util::fpa::FPA;
use log::{debug, info, warn};
use ordered_float::OrderedFloat;
use rand::prelude::SliceRandom;
use rand::rngs::SmallRng;
use rand::Rng;
use std::cmp::{min, Reverse};
use std::path::Path;
use std::process::id;
use std::time::{Duration, Instant};
use rand::distributions::WeightedIndex;
use rand::distributions::Distribution;
use tap::Tap;
use crate::sampl::eval::overlapping_evaluator::OverlappingSampleEvaluator;

const N_UNIFORM: usize = 100;
const N_CD: usize = 2;

const TIME_LIMIT: Duration = Duration::from_secs(120 * 60);

const RNG_WEIGHT_RANGE: (fsize, fsize) = (1.0, 2.0);

const N_STRIKES: usize = 5;
const N_ITER_NO_IMPROV: usize = 100;

const R_SHRINK: fsize = 0.005;
const R_EXPAND: fsize = 0.002;

pub struct GLSOptimizer {
    pub prob: SPProblem,
    pub instance: SPInstance,
    pub rng: SmallRng,
    pub ot: OverlapTracker,
    pub svg_counter: usize,
}

impl GLSOptimizer {
    pub fn new(prob: SPProblem, instance: SPInstance, rng: SmallRng) -> Self {
        let ot = OverlapTracker::new(instance.total_item_qty());
        Self {
            prob,
            instance,
            rng,
            ot,
            svg_counter: 0,
        }.initial()
    }

    fn initial(mut self) -> Self {
        while !self.prob.missing_item_qtys().iter().all(|qty| *qty == 0) {
            let weights = self.prob.missing_item_qtys().iter().enumerate()
                .map(|(id, qty)| self.instance.item(id).shape.area * (*qty as fsize).sqrt());

            let item_id = WeightedIndex::new(weights).unwrap().sample(&mut self.rng);

            let item = self.instance.item(item_id);
            let search_config = SearchConfig {
                n_bin_samples: N_UNIFORM * 10,
                n_focussed_samples: 0,
                n_coord_descents: N_CD,
            };
            let evaluator = OverlappingSampleEvaluator::new(&self.prob.layout, item, None, &self.ot);

            let (d_transf, eval) = search_placement(
                &self.prob.layout,
                item,
                None,
                evaluator,
                search_config,
                &mut self.rng,
            );
            let new_p_opt = PlacingOption {
                layout_idx: STRIP_LAYOUT_IDX,
                item_id: item.id,
                d_transf,
            };
            self.write_svg(log::LevelFilter::Debug);
            self.place_item(None, new_p_opt);
        }
        self.ot.sync(&self.prob.layout);
        self.write_svg(log::LevelFilter::Info);

        self
    }

    pub fn solve(&mut self) -> Solution {
        let mut current_width = self.prob.strip_width();
        let mut best = (current_width, self.prob.create_solution(None), self.ot.clone());

        let start = Instant::now();

        while start.elapsed() < TIME_LIMIT {
            self.separate_layout();
            self.write_svg(log::LevelFilter::Info);
            let mut next_width;
            if self.ot.get_total_overlap() == 0.0 {
                if current_width < best.0 {
                    best = (current_width, self.prob.create_solution(None), self.ot.clone());
                }
                next_width = current_width * (1.0 - R_SHRINK);
            } else {
                next_width = current_width * (1.0 + R_EXPAND);
                if next_width > best.0 {
                    next_width = current_width;
                }
            }

            if next_width != current_width {
                self.change_strip_width(next_width);
                current_width = next_width;
            }
        }

        best.1
    }

    fn separate_layout(&mut self) {
        let mut n_strikes = 0;
        let mut min_overlap = self.ot.get_total_overlap();
        let init_overlap = min_overlap;
        let mut min_sol = (self.prob.create_solution(None), self.ot.clone());
        self.ot.randomize_weights(RNG_WEIGHT_RANGE.0..RNG_WEIGHT_RANGE.1, &mut self.rng);

        while n_strikes < N_STRIKES {
            self.rollback(&min_sol.0, &min_sol.1);
            self.ot.randomize_weights(RNG_WEIGHT_RANGE.0..RNG_WEIGHT_RANGE.1, &mut self.rng);
            let init_min_overlap = min_overlap;

            let mut n_iter_no_improv = 0;
            let mut improved = false;

            while n_iter_no_improv < N_ITER_NO_IMPROV {
                let w_overlap_before = self.ot.get_total_weighted_overlap();
                let n_mov = self.modify();
                let abs_overlap = self.ot.get_total_overlap();
                let w_overlap = self.ot.get_total_weighted_overlap();
                self.write_svg(log::LevelFilter::Debug);
                info!("[i:{}]  w_o: {:.3} -> {:.3}, n_mov: {}, abs_o: {:.3} (min: {:.3}, x{:.3})",n_iter_no_improv,w_overlap_before,w_overlap,n_mov,abs_overlap,min_overlap,abs_overlap / min_overlap);
                if abs_overlap < min_overlap {
                    min_overlap = abs_overlap;
                    min_sol = (self.prob.create_solution(None), self.ot.clone());
                    improved = true;
                    n_iter_no_improv = 0;
                } else {
                    n_iter_no_improv += 1;
                }
                if abs_overlap == 0.0 {
                    warn!("separation reached zero overlap");
                    return;
                }
                self.ot.increment_weights();
            }

            if min_overlap < init_min_overlap * 0.99 {
                n_strikes = 0;
            } else {
                n_strikes += 1;
            }
            warn!(
                "strike {}/{}: {:.3} -> {:.3}",
                n_strikes, N_STRIKES, init_min_overlap, min_overlap
            );
        }

        warn!("separation improved from {:.3} to {:.3}",init_overlap, min_overlap);
        //self.rollback(&min_sol.0, &min_sol.1);
    }

    pub fn rollback(&mut self, solution: &Solution, ot: &OverlapTracker){
        self.prob.restore_to_solution(solution);
        self.ot = ot.clone();
    }

    fn modify(&mut self) -> usize {
        let overlapping_pks = self.prob.layout.placed_items.keys()
            .filter(|pk| self.ot.get_overlap(*pk) > 0.0)
            .sorted_by_key(|pk| self.ot.move_history[*pk])
            .collect_vec();

        let mut n_mov = 0;

        for pk in overlapping_pks {
            if self.ot.get_overlap(pk) > 0.0 {
                let item = self.instance.item(self.prob.layout.placed_items[pk].item_id);
                let search_config = SearchConfig {
                    n_bin_samples: N_UNIFORM / 2,
                    n_focussed_samples: N_UNIFORM / 2,
                    n_coord_descents: N_CD,
                };
                let evaluator = OverlappingSampleEvaluator::new(&self.prob.layout, item, Some(pk), &self.ot);

                let (d_transf, eval) = search_placement(
                    &self.prob.layout,
                    item,
                    Some(pk),
                    evaluator,
                    search_config,
                    &mut self.rng,
                );
                let new_p_opt = PlacingOption {
                    layout_idx: STRIP_LAYOUT_IDX,
                    item_id: item.id,
                    d_transf,
                };

                self.place_item(Some(pk), new_p_opt);
                n_mov += 1;
            }
        }
        n_mov
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
        info!("changing strip width from {:.3} to {:.3}", self.prob.strip_width(), new_width);
        let current_width = self.prob.strip_width();
        let delta = new_width - current_width - FPA::tolerance();
        let fault_pos_x = current_width / 2.0;

        let shift = (delta, 0.0);

        //create new problem with the new width
        let mut new_prob = SPProblem::new(
            self.instance.clone(),
            new_width,
            self.prob.layout.bin.base_cde.config(),
        );

        //place all the items in the new problem, shift if past the fault position
        for (_, pi) in self.prob.layout.placed_items.iter() {
            let d_transf = match pi.shape.centroid().0 < fault_pos_x {
                true => pi.d_transf,
                false => pi.d_transf.compose().translate(shift).decompose(),
            };
            new_prob.place_item(PlacingOption {
                layout_idx: STRIP_LAYOUT_IDX,
                item_id: pi.item_id,
                d_transf,
            });
        }

        self.prob = new_prob;

        self.ot = OverlapTracker::new(self.instance.total_item_qty());
        self.ot.sync(&self.prob.layout);
    }

    fn write_svg(&mut self, log_level: log::LevelFilter) {
        //skip if this log level is ignored by the logger
        if log_level > log::max_level() {
            return;
        }

        if self.svg_counter == 0 {
            //remove all .svg files from the output folder
            let _ = std::fs::remove_dir_all(SVG_OUTPUT_DIR);
            std::fs::create_dir_all(SVG_OUTPUT_DIR).unwrap();
        }

        let layout = &self.prob.layout;
        let filename = format!(
            "{}/{}_{:.2}.svg",
            SVG_OUTPUT_DIR,
            self.svg_counter,
            layout.bin.bbox().x_max
        );
        io::write_svg(
            &layout_to_svg(layout, &self.instance, DRAW_OPTIONS),
            Path::new(&filename),
        );
        self.svg_counter += 1;
        warn!("wrote layout to disk: file:///{}", filename);
    }
}