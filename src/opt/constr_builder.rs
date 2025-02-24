use crate::DRAW_OPTIONS;
use crate::sample::eval::SampleEval;
use crate::sample::eval::constructive_evaluator::ConstructiveEvaluator;
use crate::sample::search::{SearchConfig, search_placement};
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
use jagua_rs::util::config::CDEConfig;
use log::{debug, info, log, warn};
use ordered_float::OrderedFloat;
use rand::Rng;
use rand::prelude::{Distribution, SmallRng};
use std::cmp::Reverse;
use std::iter;
use std::path::Path;
use std::time::Instant;

pub struct ConstructiveBuilder {
    pub instance: SPInstance,
    pub prob: SPProblem,
    pub rng: SmallRng,
    pub search_config: SearchConfig,
}

impl ConstructiveBuilder {
    pub fn new(
        instance: SPInstance,
        cde_config: CDEConfig,
        rng: SmallRng,
        search_config: SearchConfig,
    ) -> Self {
        let strip_width_init = instance.item_area / instance.strip_height; //100% utilization
        let prob = SPProblem::new(instance.clone(), strip_width_init, cde_config);

        Self {
            instance,
            prob,
            rng,
            search_config,
        }
    }

    pub fn build(&mut self) -> Solution {
        let start = Instant::now();
        let n_items = self.instance.items().len();
        let sorted_item_indices = (0..n_items)
            .sorted_by_cached_key(|id| {
                let item_shape = self.instance.items()[*id].0.shape.as_ref();
                let convex_hull_area = item_shape.surrogate().convex_hull_area;
                let diameter = item_shape.diameter;
                Reverse(OrderedFloat(convex_hull_area * diameter))
            })
            .map(|id| {
                let missing_qty = self.prob.missing_item_qtys()[id].max(0) as usize;
                iter::repeat(id).take(missing_qty)
            })
            .flatten()
            .collect_vec();

        for item_id in sorted_item_indices {
            self.place_item(item_id);
        }

        self.prob.fit_strip();
        debug!(
            "[CONSTR] built solution in {:?}, width: {:?}",
            start.elapsed(),
            self.prob.strip_width()
        );

        self.prob.create_solution(None)
    }

    fn place_item(&mut self, item_id: usize) {
        match self.find_placement(item_id) {
            Some(p_opt) => {
                self.prob.place_item(p_opt);
                debug!(
                    "[CONSTR] placing item {}/{} with id {} at [{}]",
                    self.prob.placed_item_qtys().sum::<usize>(),
                    self.instance.total_item_qty(),
                    p_opt.item_id,
                    p_opt.d_transf
                );
            }
            None => {
                debug!(
                    "[CONSTR] failed to place item with id {}, increasing strip width",
                    item_id
                );
                self.prob
                    .modify_strip_in_back(self.prob.strip_width() * 1.2);
                self.place_item(item_id);
            }
        }
    }

    fn find_placement(&mut self, item_id: usize) -> Option<PlacingOption> {
        let layout = &self.prob.layout;
        //search for a place
        let item = self.instance.item(item_id);
        let mut evaluator = ConstructiveEvaluator::new(layout, item);

        let (d_transf, eval) = search_placement(
            layout,
            item,
            None,
            evaluator,
            self.search_config,
            &mut self.rng,
        );

        //if found add it and go to next iteration, if not, remove item type from the list
        match eval {
            SampleEval::Valid(_) => {
                let p_opt = PlacingOption {
                    layout_idx: STRIP_LAYOUT_IDX,
                    item_id,
                    d_transf,
                };
                Some(p_opt)
            }
            _ => {
                debug!("Failed to place item #{}", item_id);
                None
            }
        }
    }
}
