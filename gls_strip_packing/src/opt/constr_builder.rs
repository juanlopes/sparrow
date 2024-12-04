use crate::io::layout_to_svg::layout_to_svg;
use crate::sampl::eval::ch_corner_evaluator::ChCornerEvaluator;
use crate::sampl::eval::SampleEval;
use crate::sampl::search::{search_placement, SearchConfig};
use crate::{io, DRAW_OPTIONS, SVG_OUTPUT_DIR};
use itertools::Itertools;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::instances::strip_packing::SPInstance;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::problem_generic::{ProblemGeneric, STRIP_LAYOUT_IDX};
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use log::{debug, info, log, warn};
use rand::distributions::WeightedIndex;
use rand::prelude::{Distribution, SmallRng};
use rand::Rng;
use std::iter;
use std::path::Path;
use crate::sampl::eval::ch_edge_evaluator::ChEdgeEvaluator;

pub struct ConstructiveBuilder {
    pub instance: SPInstance,
    pub prob: SPProblem,
    pub rng: SmallRng,
    pub svg_counter: usize,
}

impl ConstructiveBuilder {
    pub fn new(prob: SPProblem, instance: SPInstance, rng: SmallRng) -> Self {
        Self {
            instance,
            prob,
            rng,
            svg_counter: 0,
        }
    }

    pub fn restore(&mut self, solution: &Solution) {
        self.prob.restore_to_solution(solution);
    }

    pub fn place(&mut self, item_id: usize, search_config: SearchConfig) -> Option<PlacingOption> {
        let layout = &self.prob.layout;
        //search for a place
        let item = self.instance.item(item_id);
        let evaluator = ChEdgeEvaluator ::new(layout, item);

        let (d_transf, eval) =
            search_placement(layout, item, None, evaluator, search_config, &mut self.rng);

        //if found add it and go to next iteration, if not, remove item type from the list
        match eval {
            SampleEval::Valid(_) => {
                let p_opt =  PlacingOption {
                    layout_idx: STRIP_LAYOUT_IDX,
                    item_id,
                    d_transf,
                };
                debug!("Placing item #{}, id: {} at {:?}",layout.placed_items().len(),item_id,d_transf);
                self.prob.place_item(p_opt.clone());
                self.write_svg(log::LevelFilter::Debug);
                Some(p_opt)
            },
            _ => {
                debug!("Failed to place item #{}", item_id);
                None
            },
        }
    }

    pub fn fill(&mut self) -> Vec<PlacingOption> {
        let mut p_opts = vec![];
        let mut item_qtys = self.prob
            .missing_item_qtys()
            .iter()
            .map(|&qty| qty.max(0) as usize)
            .collect_vec();

        while !item_qtys.iter().all(|&qty| qty == 0) {
            let weights = item_qtys
                .iter()
                .enumerate()
                .map(|(item_id, &qty)| {
                    let value = value_item(&self.instance.item(item_id).shape);
                    let qty = qty;

                    value * usize::min(1, qty) as fsize
                })
                .collect_vec();

            let dist = WeightedIndex::new(weights).unwrap();
            let item_id = dist.sample(&mut self.rng);

            let search_config = SearchConfig {
                n_bin_samples: 100,
                n_focussed_samples: 0,
                n_coord_descents: 2,
            };

            //update weights
            match self.place(item_id, search_config) {
                Some(p_opt) => {
                    p_opts.push(p_opt);
                    item_qtys[item_id] -= 1
                },
                None => item_qtys[item_id] = 0,
            }
        }

        p_opts
    }

    pub fn write_svg(&mut self, log_level: log::LevelFilter) {
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
        log!(log_level.to_level().unwrap(), "wrote layout to disk: file:///{}", filename);
    }
}

pub fn value_item(shape: &SimplePolygon) -> fsize {
    shape.area * shape.diameter()
}

pub fn value_solution(sol: &Solution) -> fsize {
    sol.layout_snapshots.iter().map(|sl|{
        sl.placed_items.iter().map(|(_, pi)| value_item(&pi.shape)).sum::<fsize>()
    }).sum()
}
