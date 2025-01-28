use std::collections::VecDeque;
use float_cmp::approx_eq;
use itertools::Itertools;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::placed_item::PlacedItem;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::PI;
use log::{info, warn};
use ordered_float::{FloatCore, OrderedFloat};

const TRANSL_DIM_FRACTION: fsize = 0.1;
const N_SIMILAR_PI_RATIO: fsize = 0.99;
const RELEVANT_CH_FRACTION_CUTOFF: fsize = 0.5;
pub struct TabuList {
    pub capacity: usize,
    pub list: Vec<Solution>,
    pub ch_area_cutoff: fsize,
    pub n_similar_limit: usize,
}

impl TabuList {
    pub fn new(capacity: usize, instance: &impl InstanceGeneric) -> Self {
        let ch_area_cutoff = instance.items().iter()
            .map(|(item, _)| item.shape.surrogate().convex_hull_area)
            .max_by_key(|&x| OrderedFloat(x))
            .unwrap() * RELEVANT_CH_FRACTION_CUTOFF;
        let n_relevant_items = instance.items().iter()
            .filter(|(item, _)| item.shape.surrogate().convex_hull_area > ch_area_cutoff)
            .map(|(_, qty)| *qty)
            .sum::<usize>();
        let n_similar_limit = (N_SIMILAR_PI_RATIO * n_relevant_items as fsize).floor() as usize;
        dbg!(capacity);
        dbg!(n_similar_limit);
        dbg!(ch_area_cutoff);
        dbg!(n_relevant_items);

        TabuList {
            ch_area_cutoff,
            n_similar_limit,
            capacity,
            list: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, sol: Solution) {
        assert!(self.list.iter().all(|s| n_similar_placements(&sol, s, self.ch_area_cutoff) <= self.n_similar_limit));
        self.list.insert(0, sol);
        if self.list.len() > self.capacity {
            self.list.pop();
        }
    }

    pub fn sol_is_tabu(&self, sol: &Solution) -> bool {
        info!("similarities: {:?} (max: {})", self.list.iter().map(|s| n_similar_placements(sol, s, self.ch_area_cutoff)).collect_vec(), self.n_similar_limit);
        self.list.iter().any(|s| n_similar_placements(sol, s, self.ch_area_cutoff) > self.n_similar_limit)
    }

    pub fn clear(&mut self) {
        self.list.clear()
    }
}

fn n_similar_placements(sol1: &Solution, sol2: &Solution, ch_area_cutoff: fsize) -> usize {
    let l1 = &sol1.layout_snapshots[0];
    let l2 = &sol2.layout_snapshots[0];

    if l1.bin.bbox().width() != l2.bin.bbox().width() || l1.placed_items.len() != l2.placed_items.len() {
        return 0;
    }

    let mut n_similar = 0;

    for pi1 in l1.placed_items.values().filter(|pi| pi.shape.surrogate().convex_hull_area > ch_area_cutoff) {
        let x_threshold = pi1.shape.bbox.width() * TRANSL_DIM_FRACTION;
        let y_theshold = pi1.shape.bbox.height() * TRANSL_DIM_FRACTION;

        let similar_exists = l2.placed_items.values()
            .filter(|pi2| pi2.shape.surrogate().convex_hull_area > ch_area_cutoff)
            .any(|pi2| placed_items_are_similar(pi1, pi2, x_threshold, y_theshold));
        if similar_exists {
            n_similar += 1;
        }
    }
    n_similar
}

fn placed_items_are_similar(pi_1: &PlacedItem, pi_2: &PlacedItem, x_threshold: fsize, y_threshold: fsize) -> bool {
    pi_1.item_id == pi_2.item_id && d_transf_are_similar(pi_1.d_transf, pi_2.d_transf, x_threshold, y_threshold)
}

fn d_transf_are_similar(dt1: DTransformation, dt2: DTransformation, x_threshold: fsize, y_threshold: fsize) -> bool {
    let x_diff = fsize::abs(dt1.translation().0 - dt2.translation().0);
    let y_diff = fsize::abs(dt1.translation().1 - dt2.translation().1);

    if x_diff < x_threshold && y_diff < y_threshold{
        let r1 = dt1.rotation() % 2.0 * PI;
        let r2 = dt2.rotation() % 2.0 * PI;
        let angle_diff = fsize::abs(r1 - r2);
        angle_diff < 1.0.to_radians()
    }
    else {
        false
    }
}

