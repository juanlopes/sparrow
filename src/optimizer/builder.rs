use crate::sample::eval::constructive_evaluator::ConstructiveEvaluator;
use crate::sample::eval::SampleEval;
use crate::sample::search::{search_placement, SearchConfig};
use itertools::Itertools;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::instances::strip_packing::SPInstance;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::problem_generic::{ProblemGeneric, STRIP_LAYOUT_IDX};
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::util::config::CDEConfig;
use log::debug;
use ordered_float::OrderedFloat;
use rand::prelude::SmallRng;
use std::cmp::Reverse;
use std::iter;
use std::time::Instant;

pub struct LBFBuilder {
    pub instance: SPInstance,
    pub prob: SPProblem,
    pub rng: SmallRng,
    pub search_config: SearchConfig,
}

impl LBFBuilder {
    pub fn new(
        instance: SPInstance,
        cde_config: CDEConfig,
        rng: SmallRng,
        search_config: SearchConfig,
    ) -> Self {
        let init_strip_width = instance.item_area / instance.strip_height; //100% utilization
        let prob = SPProblem::new(instance.clone(), init_strip_width, cde_config);

        Self {
            instance,
            prob,
            rng,
            search_config,
        }
    }

    pub fn construct(mut self) -> Self {
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

        debug!("[CONSTR] placing items in order: {:?}",sorted_item_indices);

        for item_id in sorted_item_indices {
            self.place_item(item_id);
        }

        self.prob.fit_strip();
        debug!("[CONSTR] placed all items in width: {:.3} (in {:?})",self.prob.strip_width(), start.elapsed());
        self
    }

    fn place_item(&mut self, item_id: usize) {
        match self.find_placement(item_id) {
            Some(p_opt) => {
                self.prob.place_item(p_opt);
                debug!("[CONSTR] placing item {}/{} with id {} at [{}]",self.prob.placed_item_qtys().sum::<usize>(),self.instance.total_item_qty(),p_opt.item_id,p_opt.d_transf);
            }
            None => {
                debug!("[CONSTR] failed to place item with id {}, expanding strip width",item_id);
                self.prob.modify_strip_in_back(self.prob.strip_width() * 1.2);
                self.place_item(item_id);
            }
        }
    }

    fn find_placement(&mut self, item_id: usize) -> Option<PlacingOption> {
        let layout = &self.prob.layout;
        let item = self.instance.item(item_id);
        let evaluator = ConstructiveEvaluator::new(layout, item);

        let (d_transf, eval) = search_placement(layout, item, None, evaluator, self.search_config, &mut self.rng, );

        if let SampleEval::Valid(_) = eval {
            Some(
                PlacingOption {
                    layout_idx: STRIP_LAYOUT_IDX,
                    item_id,
                    d_transf,
                }
            )
        }
        else {
            None
        }
    }
}
