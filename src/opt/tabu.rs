use crate::sample;
use float_cmp::approx_eq;
use itertools::Itertools;
use jagua_rs::PI;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::placed_item::PlacedItem;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::Shape;
use log::{debug, info, warn};
use ordered_float::{FloatCore, OrderedFloat};
use std::collections::VecDeque;

const TRANSL_DIM_FRACTION: fsize = 0.1;
const N_SIMILAR_PI_RATIO: fsize = 0.99;
const RELEVANT_CH_FRACTION_CUTOFF: fsize = 0.5;
pub struct TabuList {
    pub capacity: usize,
    pub list: Vec<(Solution, fsize)>,
    pub ch_area_cutoff: fsize,
    pub n_similar_limit: usize,
}

impl TabuList {
    pub fn new(capacity: usize, instance: &impl InstanceGeneric) -> Self {
        let ch_area_cutoff = instance
            .items()
            .iter()
            .map(|(item, _)| item.shape.surrogate().convex_hull_area)
            .max_by_key(|&x| OrderedFloat(x))
            .unwrap()
            * RELEVANT_CH_FRACTION_CUTOFF;
        let n_relevant_items = instance
            .items()
            .iter()
            .filter(|(item, _)| item.shape.surrogate().convex_hull_area > ch_area_cutoff)
            .map(|(_, qty)| *qty)
            .sum::<usize>();
        let n_similar_limit = (N_SIMILAR_PI_RATIO * n_relevant_items as fsize).floor() as usize;

        TabuList {
            ch_area_cutoff,
            n_similar_limit,
            capacity,
            list: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, sol: Solution, eval: fsize) {
        assert!(self.list.iter().all(
            |(s, _)| n_similar_placements(&sol, s, self.ch_area_cutoff) <= self.n_similar_limit
        ));
        self.list.insert(0, (sol, eval));
        if self.list.len() > self.capacity {
            self.list.pop();
        }
    }

    pub fn sol_is_tabu(&self, sol: &Solution) -> bool {
        debug!(
            "[TABU] similarities: {:?} (max: {})",
            self.list
                .iter()
                .map(|(s, _)| n_similar_placements(sol, s, self.ch_area_cutoff))
                .collect_vec(),
            self.n_similar_limit
        );
        self.list
            .iter()
            .any(|(s, _)| n_similar_placements(sol, s, self.ch_area_cutoff) > self.n_similar_limit)
    }

    pub fn clear(&mut self) {
        self.list.clear()
    }
}

fn n_similar_placements(sol1: &Solution, sol2: &Solution, ch_area_cutoff: fsize) -> usize {
    let l1 = &sol1.layout_snapshots[0];
    let l2 = &sol2.layout_snapshots[0];

    if l1.bin.bbox().width() != l2.bin.bbox().width()
        || l1.placed_items.len() != l2.placed_items.len()
    {
        return 0;
    }

    let mut n_similar = 0;

    for pi1 in l1
        .placed_items
        .values()
        .filter(|pi| pi.shape.surrogate().convex_hull_area > ch_area_cutoff)
    {
        let x_threshold = pi1.shape.bbox.width() * TRANSL_DIM_FRACTION;
        let y_theshold = pi1.shape.bbox.height() * TRANSL_DIM_FRACTION;

        let similar_exists = l2
            .placed_items
            .values()
            .filter(|pi2| pi2.shape.surrogate().convex_hull_area > ch_area_cutoff)
            .any(|pi2| placed_items_are_similar(pi1, pi2, x_threshold, y_theshold));
        if similar_exists {
            n_similar += 1;
        }
    }
    n_similar
}

fn placed_items_are_similar(
    pi_1: &PlacedItem,
    pi_2: &PlacedItem,
    x_threshold: fsize,
    y_threshold: fsize,
) -> bool {
    pi_1.item_id == pi_2.item_id
        && sample::dtransfs_are_similar(pi_1.d_transf, pi_2.d_transf, x_threshold, y_threshold)
}
