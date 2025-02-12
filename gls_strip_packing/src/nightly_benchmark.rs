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
use numfmt::{Formatter, Precision, Scales};


const RNG_SEED: Option<usize> = None;

const N_RUNS_TOTAL: usize = 16;
const N_PARALLEL_RUNS: usize = 8;

const TIME_LIMIT_S: u64 = 20 * 60;

fn main() {

    //the input file is the first argument
    let args: Vec<String> = std::env::args().collect();
    let json_instance = io::read_json_instance(Path::new(&args[1]));

    println!("[BENCH] Starting benchmark for {} ({} runs, {} parallel, {}s timelimit)", &args[1], N_RUNS_TOTAL, N_PARALLEL_RUNS, TIME_LIMIT_S);

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

    let mut final_solutions = vec![];
    let mut n_iterations = (N_RUNS_TOTAL as fsize / N_PARALLEL_RUNS as fsize).ceil() as usize;

    for i in 0..n_iterations {
        println!("[BENCH] Starting iter {}/{}", i + 1, n_iterations);
        let mut iter_solutions = vec![None; N_PARALLEL_RUNS];
        rayon::scope(|s| {
            for (j, solution_slice) in iter_solutions.iter_mut().enumerate(){
                let thread_rng = SmallRng::seed_from_u64(rng.gen());
                let svg_output_dir = format!("{}_{}", SVG_OUTPUT_DIR, i * N_PARALLEL_RUNS + j);
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
        final_solutions.extend(iter_solutions.into_iter().flatten());
    }

    //print statistics about the solutions, print best, worst, median and average
    let (mut final_widths, mut final_usages): (Vec<fsize>, Vec<fsize>) = final_solutions.into_iter()
        .map(|s| {
            let width = s.layout_snapshots[0].bin.bbox().width();
            let usage = s.layout_snapshots[0].usage;
            (width, usage * 100.0)
        })
        .sorted_by_key(|(w,u)| OrderedFloat(*w))
        .unzip();

    let n_results = final_widths.len();

    let avg_width = final_widths.iter().sum::<fsize>() / n_results as fsize;
    let stddev_width = (final_widths.iter().map(|w| (w - avg_width).powi(2)).sum::<fsize>() / n_results as fsize).sqrt();

    println!("Benchmarked {} with {} runs ({}s)", args[1], n_results, TIME_LIMIT_S);
    println!("Results: {:?}", final_widths);

    println!("----------------- WIDTH -----------------");
    println!("Best: {}", final_widths.first().unwrap());
    println!("Worst: {}", final_widths.last().unwrap());
    println!("Med: {}", final_widths[final_widths.len() / 2]);
    println!("Avg: {}", avg_width);
    println!("Stddev: {}", stddev_width);

    let avg_yield = final_usages.iter().sum::<fsize>() / n_results as fsize;
    let stddev_yield = (final_usages.iter().map(|u| (u - avg_yield).powi(2)).sum::<fsize>() / n_results as fsize).sqrt();

    println!("----------------- USAGE -----------------");
    println!("Best: {}", final_usages.first().unwrap());
    println!("Worst: {}", final_usages.last().unwrap());
    println!("Med: {}", final_usages[final_usages.len() / 2]);
    println!("Avg: {}", avg_yield);
    println!("Stddev: {}", stddev_yield);
}
