/*
    General idea: during the last 20% of time, take very small steps to reduce the bin size. Aggressively restoring from previous solutions.
    Run this on top of gls_orchestrator.rs, do a random split position
 */
use std::time::Instant;
use jagua_rs::entities::problems::strip_packing::strip_width;
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use log::{debug, info};
use rand::Rng;
use crate::config::{POST_N_ATTEMPTS, POST_R_SHRINKS};
use crate::opt::gls_orchestrator::GLSOrchestrator;

pub fn post_optimize(gls: &mut GLSOrchestrator, init: &Solution) -> Solution {
    let mut best = init.clone();
    for r_shrink in POST_R_SHRINKS {
        let mut n_attempts = 0;
        info!("[POST] [t:{}] attempting to reduce width by {}%", n_attempts, r_shrink * 100.0);
        while n_attempts < POST_N_ATTEMPTS {
            match compact(gls, &best, r_shrink){
                Some(compacted_sol) => {
                    assert!(compacted_sol.usage > best.usage);
                    info!("[POST] reached improved solution with {} width ({:.3}%)", strip_width(&compacted_sol), compacted_sol.usage * 100.0);
                    gls.write_to_disk(Some(compacted_sol.clone()), "p", true);
                    best = compacted_sol;
                    n_attempts = 0;
                }
                None => n_attempts += 1
            }
        }
    }
    info!("[POST] finished compacted from {:.3}% to {:.3}% (+{:.3}%)", init.usage * 100.0, best.usage * 100.0, (init.usage - best.usage) * 100.0);
    best
}


fn compact(gls: &mut GLSOrchestrator, init: &Solution, r_shrink: fsize) -> Option<Solution> {
    //restore to the initial solution and width
    gls.change_strip_width(strip_width(&init), None);
    gls.rollback(&init, None);

    let new_width = strip_width(init) * (1.0 - r_shrink);
    let split_pos = gls.rng.random_range(0.0..gls.prob.strip_width());

    gls.change_strip_width(new_width, Some(split_pos));

    let (compacted_sol, ot) = gls.separate_layout(None);
    match ot.total_overlap == 0.0 {
        true => Some(compacted_sol),
        false => None,
    }
}