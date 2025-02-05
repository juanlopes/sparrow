extern crate core;

use std::path::Path;
use std::time::{Duration, Instant};
use itertools::Itertools;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::fsize;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use log::{info, warn};
use mimalloc::MiMalloc;
use once_cell::sync::Lazy;
use ordered_float::OrderedFloat;
use rand::prelude::SmallRng;
use rand::{Rng, SeedableRng};
use gls_strip_packing::{io, SVG_OUTPUT_DIR};
use gls_strip_packing::opt::constr_builder::ConstructiveBuilder;
use gls_strip_packing::opt::gls_optimizer::GLSOptimizer;
use gls_strip_packing::opt::gls_orchestrator::GLSOrchestrator;
use gls_strip_packing::sample::search::SearchConfig;

const INPUT_FILE: &str = "../jagua-rs/assets/albano.json";

const RNG_SEED: Option<usize> = Some(0);

const N_PARALLEL_RUNS: usize = 4;

const TIME_LIMIT_S: u64 = 20 * 60;

fn main() {

    if cfg!(debug_assertions) {
        io::init_logger(log::LevelFilter::Debug);
    }
    else {
        io::init_logger(log::LevelFilter::Info);
    }

    let json_instance = io::read_json_instance(Path::new(&INPUT_FILE));

    let cde_config = CDEConfig{
        quadtree_depth: 4,
        hpg_n_cells: 2000,
        item_surrogate_config: SPSurrogateConfig {
            pole_coverage_goal: 0.95,
            max_poles: 20,
            n_ff_poles: 2,
            n_ff_piers: 0,
        },
    };

    let parser = Parser::new(PolySimplConfig::Disabled, cde_config, true);
    let instance = parser.parse(&json_instance);

    let sp_instance = match instance.clone(){
        Instance::SP(spi) => spi,
        _ => panic!("Expected SPInstance"),
    };

    let mut rng = match RNG_SEED {
        Some(seed) => SmallRng::seed_from_u64(seed as u64),
        None => {
            let seed = rand::random();
            warn!("No seed provided, using: {}", seed);
            SmallRng::seed_from_u64(seed)
        }
    };

    let constr_search_config = SearchConfig{
        n_bin_samples: 1000,
        n_focussed_samples: 0,
        n_coord_descents: 3,
    };

    let mut best_solutions = vec![None; N_PARALLEL_RUNS];

    rayon::scope(|s| {
        for (i, solution_slice) in best_solutions.iter_mut().enumerate(){
            let thread_rng = SmallRng::seed_from_u64(rng.gen());
            let svg_output_dir = format!("{}_{}", SVG_OUTPUT_DIR, i);
            let instance = sp_instance.clone();

            s.spawn(|_| {
                let mut constr_builder = ConstructiveBuilder::new(instance, cde_config, thread_rng, constr_search_config);
                constr_builder.build();

                let instance = constr_builder.instance;
                let problem = constr_builder.prob;
                let rng = constr_builder.rng;

                let mut gls_opt = GLSOrchestrator::new(problem, instance, rng, svg_output_dir);

                let solution = gls_opt.solve(Duration::from_secs(TIME_LIMIT_S));

                *solution_slice = Some(solution);
            })
        }
    });

    //print statistics about the solutions, print best, worst, median and average
    let mut best_widths = best_solutions.into_iter()
        .map(|s| s.unwrap().layout_snapshots[0].bin.bbox().width())
        .sorted_by_key(|w| OrderedFloat(*w))
        .collect_vec();

    let best_width = best_widths.first().unwrap();
    let worst_width = best_widths.last().unwrap();
    let median_width = best_widths[best_widths.len() / 2];
    let avg_width = best_widths.iter().sum::<fsize>() / best_widths.len() as fsize;
    let stdev_width = best_widths.iter().map(|w| (w - avg_width).powi(2)).sum::<fsize>().sqrt();

    info!("Benchmarked {} with {} runs", INPUT_FILE, N_PARALLEL_RUNS);
    info!("Best width: {}", best_widths.first().unwrap());
    info!("Worst width: {}", best_widths.last().unwrap());
    info!("Median width: {}", best_widths[best_widths.len() / 2]);
    info!("Average width: {}", best_widths.iter().sum::<fsize>() / best_widths.len() as fsize);
    info!("Standard deviation: {}", stdev_width);

    println!("Hello, world!");
}
