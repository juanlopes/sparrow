use crate::config::*;
use crate::optimizer::lbf::LBFBuilder;
use crate::optimizer::separator::Separator;
pub use crate::optimizer::terminator::Terminator;
use crate::sample::uniform_sampler::{convert_sample_to_closest_feasible, UniformBBoxSampler};
use crate::FMT;
use float_cmp::approx_eq;
use itertools::Itertools;
use jagua_rs::entities::{Instance, Layout};
use jagua_rs::probs::spp::entities::{SPInstance, SPSolution};
use log::info;
use ordered_float::OrderedFloat;
use rand::prelude::{IteratorRandom, SmallRng};
use rand::{random_range, Rng, RngCore, SeedableRng};
use rand_distr::Distribution;
use rand_distr::Normal;
use std::iter;
use std::time::{Duration, Instant};
use jagua_rs::collision_detection::hazards::detector::{BasicHazardDetector, HazardDetector};
use jagua_rs::collision_detection::hazards::HazardEntity;
use jagua_rs::geometry::geo_traits::CollidesWith;
use jagua_rs::geometry::primitives::Rect;
use rand::distr::Uniform;

pub mod lbf;
pub mod separator;
mod worker;
pub mod terminator;

// All high-level heuristic logic
pub fn optimize(instance: SPInstance, mut rng: SmallRng, output_folder_path: String, mut terminator: Terminator, explore_dur: Duration, compress_dur: Duration) -> SPSolution {
    let mut next_rng = || SmallRng::seed_from_u64(rng.next_u64());
    let builder = LBFBuilder::new(instance.clone(), next_rng(), LBF_SAMPLE_CONFIG).construct();

    terminator.set_timeout_from_now(explore_dur);
    let mut expl_separator = Separator::new(builder.instance, builder.prob, next_rng(), output_folder_path.clone(), 0, SEP_CFG_EXPLORE);
    let solutions = exploration_phase(&instance, &mut expl_separator, &terminator);
    let final_explore_sol = solutions.last().unwrap().clone();

    terminator.set_timeout_from_now(compress_dur).reset_ctrlc();
    let mut cmpr_separator = Separator::new(expl_separator.instance, expl_separator.prob, next_rng(), expl_separator.output_svg_folder, expl_separator.svg_counter, SEP_CFG_COMPRESS);
    let cmpr_sol = compression_phase(&instance, &mut cmpr_separator, &final_explore_sol, &terminator);

    cmpr_sol
}

pub fn exploration_phase(instance: &SPInstance, sep: &mut Separator, term: &Terminator) -> Vec<SPSolution> {
    let mut current_width = sep.prob.strip_width();
    let mut best_width = current_width;

    let mut feasible_solutions = vec![sep.prob.save()];

    sep.export_svg(None, "init", false);
    info!("[EXPL] starting optimization with initial width: {:.3} ({:.3}%)",current_width,sep.prob.density() * 100.0);

    let mut solution_pool: Vec<(SPSolution, f32)> = vec![];

    while !term.is_kill() {
        let local_best = sep.separate(&term);
        let total_loss = local_best.1.get_total_loss();

        if total_loss == 0.0 {
            //layout is successfully separated
            if current_width < best_width {
                info!("[EXPL] new best at width: {:.3} ({:.3}%)",current_width,sep.prob.density() * 100.0);
                best_width = current_width;
                feasible_solutions.push(local_best.0.clone());
                sep.export_svg(Some(local_best.0.clone()), "expl_f", false);
            }
            let next_width = current_width * (1.0 - EXPLORE_SHRINK_STEP);
            info!("[EXPL] shrinking width by {}%: {:.3} -> {:.3}", EXPLORE_SHRINK_STEP * 100.0, current_width, next_width);
            sep.change_strip_width(next_width, None);
            current_width = next_width;
            solution_pool.clear();
        } else {
            info!("[EXPL] layout separation unsuccessful, exporting min loss solution");
            sep.export_svg(Some(local_best.0.clone()), "expl_nf", false);

            //layout was not successfully separated, add to local bests
            match solution_pool.binary_search_by(|(_, o)| o.partial_cmp(&total_loss).unwrap()) {
                Ok(idx) | Err(idx) => solution_pool.insert(idx, (local_best.0.clone(), total_loss)),
            }

            //restore to a random solution from the tabu list, better solutions have more chance to be selected
            let selected_sol = {
                //sample a value in range [0.0, 1.0[ from a normal distribution
                let distr = Normal::new(0.0, EXPLORE_SOL_DISTR_STDDEV).unwrap();
                let sample = distr.sample(&mut sep.rng).abs().min(0.999);
                //map it to the range of the solution pool
                let selected_idx = (sample * solution_pool.len() as f32) as usize;

                let (selected_sol, loss) = &solution_pool[selected_idx];
                info!("[EXPL] selected starting solution {}/{} from solution pool (l: {})", selected_idx, solution_pool.len(), FMT().fmt2(*loss));
                selected_sol
            };

            //restore and swap two large items
            sep.rollback(selected_sol, None);
            //swap_large_pair_of_items(sep);
            move_large_item(sep)
        }
    }

    info!("[EXPL] time limit reached, best solution found: {:.3} ({:.3}%)",best_width,feasible_solutions.last().unwrap().density(instance) * 100.0);

    feasible_solutions
}

pub fn compression_phase(instance: &SPInstance, sep: &mut Separator, init: &SPSolution, term: &Terminator) -> SPSolution {
    let mut best = init.clone();
    let start = Instant::now();
    let end = term.timeout.expect("compression running without timeout");
    let step_size = || -> f32 {
        //map the range [COMPRESS_SHRINK_RANGE.0, COMPRESS_SHRINK_RANGE.1] to timeout
        let range = COMPRESS_SHRINK_RANGE.1 - COMPRESS_SHRINK_RANGE.0;
        let elapsed = start.elapsed();
        let remaining = end.duration_since(Instant::now());
        let ratio = elapsed.as_secs_f32() / (elapsed + remaining).as_secs_f32();
        COMPRESS_SHRINK_RANGE.0 + ratio * range
    };
    while !term.is_kill() {
        let step = step_size();
        info!("[CMPR] attempting {:.3}%", step * 100.0);
        match attempt_to_compress(sep, &best, step, &term) {
            Some(compacted_sol) => {
                info!("[CMPR] compressed to {:.3} ({:.3}%)", compacted_sol.strip_width(), compacted_sol.density(instance) * 100.0);
                sep.export_svg(Some(compacted_sol.clone()), "cmpr", false);
                best = compacted_sol;
            }
            None => {}
        }
    }
    info!("[CMPR] finished compression, improved from {:.3}% to {:.3}% (+{:.3}%)", init.density(instance) * 100.0, best.density(instance) * 100.0, (best.density(instance) - init.density(instance)) * 100.0);
    best
}


fn attempt_to_compress(sep: &mut Separator, init: &SPSolution, r_shrink: f32, term: &Terminator) -> Option<SPSolution> {
    //restore to the initial solution and width
    sep.change_strip_width(init.strip_width(), None);
    sep.rollback(&init, None);

    //shrink the container at a random position
    let new_width = init.strip_width() * (1.0 - r_shrink);
    let split_pos = sep.rng.random_range(0.0..sep.prob.strip_width());
    sep.change_strip_width(new_width, Some(split_pos));

    //try to separate layout, if all collisions are eliminated, return the solution
    let (compacted_sol, ot) = sep.separate(term);
    match ot.get_total_loss() == 0.0 {
        true => Some(compacted_sol),
        false => None,
    }
}

fn swap_large_pair_of_items(sep: &mut Separator) {
    //TODO: make a more elaborate way of selecting between significant and non-significant items
    //      to make the disruption more robust across instances

    let ascending_ch_areas = sep.prob.instance.items.iter()
        .sorted_by_key(|(item, _)| OrderedFloat(item.shape_cd.surrogate().convex_hull_area))
        .rev()
        .map(|(i, q)| iter::repeat(i.shape_cd.surrogate().convex_hull_area).take(*q))
        .flatten()
        .collect_vec();

    //Calculate the convex hull area of the LARGE_AREA_CH_AREA_CUTOFF_PERCENTILE item
    let idx = (ascending_ch_areas.len() as f32 * LARGE_AREA_CH_AREA_CUTOFF_PERCENTILE) as usize;
    let large_area_ch_area_cutoff = ascending_ch_areas[idx];


    let layout = &sep.prob.layout;

    //Choose a first item with a large enough convex hull
    let (pk1, pi1) = layout.placed_items.iter()
        .filter(|(_, pi)| pi.shape.surrogate().convex_hull_area > large_area_ch_area_cutoff)
        .choose(&mut sep.rng)
        .unwrap();

    //Choose a second item with a large enough convex hull and different enough from the first.
    //If no such item is found, choose a random one.
    let (pk2, pi2) = layout.placed_items.iter()
        .filter(|(_, pi)| !approx_eq!(f32, pi.shape.area,pi1.shape.area, epsilon = pi1.shape.area * 0.1))
        .filter(|(_, pi)| pi.shape.surrogate().convex_hull_area > large_area_ch_area_cutoff)
        .choose(&mut sep.rng)
        .unwrap_or(layout.placed_items.iter()
            .filter(|(pk2, _)| *pk2 != pk1)
            .choose(&mut sep.rng).unwrap());

    let dt1 = convert_sample_to_closest_feasible(pi2.d_transf, sep.prob.instance.item(pi1.item_id));
    let dt2 = convert_sample_to_closest_feasible(pi1.d_transf, sep.prob.instance.item(pi2.item_id));

    info!("[EXPL] swapped two large items (ids: {} <-> {})", pi1.item_id, pi2.item_id);

    sep.move_item(pk1, dt1);
    sep.move_item(pk2, dt2);
}

fn move_large_item(sep: &mut Separator) {
    //sep.export_svg(None, "before_disruption", false);

    let ascending_ch_areas = sep.prob.instance.items.iter()
        .sorted_by_key(|(item, _)| OrderedFloat(item.shape_cd.surrogate().convex_hull_area))
        .rev()
        .map(|(i, q)| iter::repeat(i.shape_cd.surrogate().convex_hull_area).take(*q))
        .flatten()
        .collect_vec();
    
    //Calculate the convex hull area of the LARGE_AREA_CH_AREA_CUTOFF_PERCENTILE item
    let idx = (ascending_ch_areas.len() as f32 * LARGE_AREA_CH_AREA_CUTOFF_PERCENTILE) as usize;
    let large_area_ch_area_cutoff = ascending_ch_areas[idx];

    // let large_area_ch_area_cutoff = sep.instance.items()
    //     .map(|item| item.shape_cd.surrogate().convex_hull_area)
    //     .max_by_key(|&x| OrderedFloat(x))
    //     .unwrap() * LARGE_AREA_CH_AREA_CUTOFF_PERCENTILE;

    let layout = &sep.prob.layout;

    //Choose an item with a large enough convex hull
    let (pk, pi) = layout.placed_items.iter()
        .filter(|(_, pi)| pi.shape.surrogate().convex_hull_area > large_area_ch_area_cutoff)
        .choose(&mut sep.rng)
        .unwrap();

    let dt = pi.d_transf;
    
    //Choose a random place to move it to
    let sampler = UniformBBoxSampler::new(
        sep.prob.layout.cde().bbox(),
        sep.instance.item(pi.item_id),
        sep.prob.layout.cde().bbox()
    ).unwrap();
    let new_dt = sampler.sample(&mut sep.rng);

    //Move it there
    let new_pk = sep.move_item(pk, new_dt);

    //sep.export_svg(None, "during_disruption", false);

    let colliding_items = {
        let layout = &sep.prob.layout;
        let new_pi = &layout.placed_items[new_pk];
        let cde = layout.cde();
        let mut hazard_detector = BasicHazardDetector::new();
        layout.cde().collect_poly_collisions(&new_pi.shape, &mut hazard_detector);
        hazard_detector.iter()
            .filter_map(|he| {
                match he {
                    HazardEntity::PlacedItem{pk, ..} => {
                        if *pk == new_pk {
                            //Skip the item itself
                            return None;
                        }
                        else {
                            Some(*pk)
                        }
                    },
                    _ => None
                }
            })
            .collect_vec()
    };

    //dbg!(&colliding_items.len());
    
    let moved_item_bbox = sep.prob.layout.placed_items[new_pk].shape.bbox;
    
    let new_t = new_dt.compose();
    let inv_new_t = new_t.clone().inverse();
    let t = dt.compose();
    let inv_t = t.clone().inverse();

    //Move all colliding items to the "empty space" created by the moved item
    for c_pk in colliding_items {
        let c_pi = &sep.prob.layout.placed_items[c_pk];
        let c_pi_bbox = c_pi.shape.bbox;
        let intersect_area = Rect::intersection(moved_item_bbox, c_pi_bbox).expect("colliding items should have intersecting bounding boxes").area();
        //only move smaller items
        if intersect_area > f32::min(moved_item_bbox.area(), c_pi_bbox.area()) * 0.5 {
            let new_dt = c_pi.d_transf.compose().transform(&inv_new_t).transform(&t).decompose();
            //make sure the new position is feasible
            let new_feasible_dt = convert_sample_to_closest_feasible(new_dt, sep.prob.instance.item(c_pi.item_id));
            //dbg!(new_dt, new_feasible_dt);

            sep.move_item(c_pk, new_feasible_dt);
        }
    }

    //sep.export_svg(None, "after_disruption", false);
}