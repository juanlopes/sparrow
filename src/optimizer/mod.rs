use std::time::{Duration, Instant};
use jagua_rs::entities::instances::strip_packing::SPInstance;
use jagua_rs::entities::problems::problem_generic::ProblemGeneric;
use jagua_rs::entities::problems::strip_packing::strip_width;
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use log::{info, Level};
use rand::prelude::SmallRng;
use rand::Rng;
use rand_distr::Normal;
use rand_distr::Distribution;
use crate::config::{CDE_CONFIG, CONSTR_SEARCH_CONFIG, EXPLORE_SOL_DISTR_STDDEV, COMPRESS_N_STRIKES, COMPRESS_R_SHRINKS, EXPLORE_R_SHRINK, SEPARATOR_CONFIG_COMPRESS, SEPARATOR_CONFIG_EXPLORE};
use crate::FMT;
use crate::optimizer::builder::LBFBuilder;
use crate::optimizer::separator::Separator;
pub mod builder;
pub mod separator;
mod separator_worker;

// All high-level heuristic logic
pub fn optimize(instance: SPInstance, rng: SmallRng, output_folder_path: String, explore_time_limit: Duration) -> Solution {
    let builder = LBFBuilder::new(instance, CDE_CONFIG, rng, CONSTR_SEARCH_CONFIG).construct();
    let mut expl_separator = Separator::new(builder.instance, builder.prob, builder.rng, output_folder_path.clone(), 0, SEPARATOR_CONFIG_EXPLORE);

    let solutions = explore(&mut expl_separator, explore_time_limit);
    let final_explore_sol = solutions.last().expect("no solutions found during exploration");

    let mut cmpr_separator = Separator::new(expl_separator.instance, expl_separator.prob, expl_separator.rng, expl_separator.output_svg_folder, expl_separator.svg_counter, SEPARATOR_CONFIG_COMPRESS);

    let final_sol = compress(&mut cmpr_separator, final_explore_sol);

    final_sol
}

pub fn explore(sep: &mut Separator, time_out: Duration) -> Vec<Solution> {
    let mut current_width = sep.prob.occupied_width();
    let mut best_width = current_width;

    let mut feasible_solutions = vec![sep.prob.create_solution(None)];

    sep.export_svg(None, "init", false);
    info!("[EXPL] starting optimization with initial width: {:.3} ({:.3}%)",current_width,sep.prob.usage() * 100.0);

    let end_time = Instant::now() + time_out;
    let mut solution_pool: Vec<(Solution, fsize)> = vec![];

    while Instant::now() < end_time {
        let local_best = sep.separate_layout(Some(end_time));
        let total_overlap = local_best.1.total_overlap;

        if total_overlap == 0.0 {
            //layout is successfully separated
            if current_width < best_width {
                info!("[EXPL] new best at width: {:.3} ({:.3}%)",current_width,sep.prob.usage() * 100.0);
                best_width = current_width;
                feasible_solutions.push(local_best.0.clone());
                sep.export_svg(Some(local_best.0.clone()), "f", false);
            }
            let next_width = current_width * (1.0 - EXPLORE_R_SHRINK);
            info!("[EXPL] shrinking width by {}%: {:.3} -> {:.3}", EXPLORE_R_SHRINK * 100.0, current_width, next_width);
            sep.change_strip_width(next_width, None);
            current_width = next_width;
            solution_pool.clear();
        } else {
            info!("[EXPL] layout separation unsuccessful, exporting min overlap solution");
            sep.export_svg(Some(local_best.0.clone()), "o", false);

            //layout was not successfully separated, add to local bests
            match solution_pool.binary_search_by(|(_, o)| o.partial_cmp(&total_overlap).unwrap()) {
                Ok(idx) | Err(idx) => solution_pool.insert(idx, (local_best.0.clone(), total_overlap)),
            }

            //restore to a random solution from the tabu list, better solutions have more chance to be selected
            let selected_sol = {
                //sample a value in range [0.0, 1.0[ from a normal distribution
                let distr = Normal::new(0.0, EXPLORE_SOL_DISTR_STDDEV).unwrap();
                let sample = distr.sample(&mut sep.rng).abs().min(0.999);
                //map it to the range of the solution pool
                let selected_idx = (sample * solution_pool.len() as fsize) as usize;

                let (selected_sol, overlap) = &solution_pool[selected_idx];
                info!("[EXPL] selected starting solution {}/{} from solution pool (o: {})", selected_idx, solution_pool.len(), FMT.fmt2(*overlap));
                selected_sol
            };

            //restore and swap two large items
            sep.rollback(selected_sol, None);
            sep.swap_large_pair_of_items();
        }
    }

    info!("[EXPL] time limit reached, best solution found: {:.3} ({:.3}%)",best_width,feasible_solutions.last().unwrap().usage * 100.0);

    feasible_solutions
}

pub fn compress(sep: &mut Separator, init: &Solution) -> Solution {
    let mut best = init.clone();
    for (i, &r_shrink) in COMPRESS_R_SHRINKS.iter().enumerate() {
        let mut n_strikes = 0;
        info!("[CMPR] attempting to compress in steps of {}%", r_shrink * 100.0);
        while n_strikes < COMPRESS_N_STRIKES[i] {
            match try_compress(sep, &best, r_shrink) {
                Some(compacted_sol) => {
                    info!("[CMPR] compressed to {:.3} ({:.3}%)", strip_width(&compacted_sol), compacted_sol.usage * 100.0);
                    sep.export_svg(Some(compacted_sol.clone()), "p", false);
                    best = compacted_sol;
                    n_strikes = 0;
                }
                None => {
                    n_strikes += 1;
                    info!("[CMPR] strike {}/{}", n_strikes, COMPRESS_N_STRIKES[i]);
                }
            }
        }
    }
    info!("[CMPR] finished compression, improved from {:.3}% to {:.3}% (+{:.3}%)", init.usage * 100.0, best.usage * 100.0, (best.usage - init.usage) * 100.0);
    best
}


//TODO: refine separator for this purpose (more greedy, more restores)
fn try_compress(sep: &mut Separator, init: &Solution, r_shrink: fsize) -> Option<Solution> {
    //restore to the initial solution and width
    sep.change_strip_width(strip_width(&init), None);
    sep.rollback(&init, None);

    //shrink the bin at a random position
    let new_width = strip_width(init) * (1.0 - r_shrink);
    let split_pos = sep.rng.random_range(0.0..sep.prob.strip_width());
    sep.change_strip_width(new_width, Some(split_pos));

    //separate layout
    let (compacted_sol, ot) = sep.separate_layout(None);
    match ot.total_overlap == 0.0 {
        true => Some(compacted_sol),
        false => None,
    }
}