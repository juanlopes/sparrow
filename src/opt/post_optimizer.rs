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
use crate::opt::gls_orchestrator::{GLSOrchestrator, R_SHRINK};

pub const SHRINK_STEP: fsize = R_SHRINK / 10.0; // one tenth of the normal shrink

pub fn compact(gls: &mut GLSOrchestrator, init: &Solution, time_out: Instant) -> Solution {
    //restore to the initial solution and width
    gls.change_strip_width(strip_width(init), None);
    gls.rollback(&init, None);

    let new_width = gls.master_prob.strip_width() * (1.0 - SHRINK_STEP);

    let split_pos = gls.rng.random_range(0.0..gls.master_prob.strip_width());
    gls.change_strip_width(new_width, Some(split_pos));
    info!("[POST] attempting to reduce width to {}", new_width);

    let (compacted_sol, ot, _) = gls.separate_layout(time_out);
    let best_feasible_sol = match ot.total_overlap == 0.0 {
        true => {
            info!("[POST] reached improved solution with {} width ({:.3}%)", new_width, compacted_sol.usage);
            gls.write_to_disk(Some(compacted_sol.clone()), "post_c", true);
            compacted_sol
        },
        false => {
            gls.write_to_disk(Some(compacted_sol.clone()), "post_o", false);
            debug!("[POST] no improvement, returning initial solution");
            init.clone()
        },
    };

    match Instant::now() < time_out {
        true => compact(gls, &best_feasible_sol, time_out),
        false => best_feasible_sol,
    }
}