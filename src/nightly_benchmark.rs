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
use gls_strip_packing::SVG_OUTPUT_DIR;
use gls_strip_packing::util::io;
use gls_strip_packing::opt::constr_builder::ConstructiveBuilder;
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

    println!("Starting benchmark for {} ({} runs, {} parallel, {}s timelimit)", &args[1], N_RUNS_TOTAL, N_PARALLEL_RUNS, TIME_LIMIT_S);

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
        Some(seed) => {
            println!("Using seed: {}", seed);
            SmallRng::seed_from_u64(seed as u64)
        },
        None => {
            let seed = rand::random();
            println!("No seed provided, using: {}", seed);
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
        println!("Starting iter {}/{}", i + 1, n_iterations);
        let mut iter_solutions = vec![None; N_PARALLEL_RUNS];
        rayon::scope(|s| {
            for (j, solution_slice) in iter_solutions.iter_mut().enumerate(){
                let thread_rng = SmallRng::seed_from_u64(rng.random());
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
        }).unzip();

    let n_results = final_widths.len();

    dbg!(&final_widths);
    dbg!(&final_usages);

    println!("----------------- WIDTH -----------------");
    println!("Worst: {}", final_widths.iter().max_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("25per: {}", calculate_percentile(&final_widths, 0.75));
    println!("Med: {}", calculate_median(&final_widths));
    println!("75per: {}", calculate_percentile(&final_widths, 0.25));
    println!("Best: {}", final_widths.iter().min_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("Avg: {}", calculate_average(&final_widths));
    println!("Stddev: {}", calculate_stddev(&final_widths));
    println!();
    println!("----------------- USAGE -----------------");
    println!("Worst: {}", final_usages.iter().min_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("25per: {}", calculate_percentile(&final_usages, 0.25));
    println!("Median: {}", calculate_median(&final_usages));
    println!("75per: {}", calculate_percentile(&final_usages, 0.75));
    println!("Best: {}", final_usages.iter().max_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("Avg: {}", calculate_average(&final_usages));
    println!("Stddev: {}", calculate_stddev(&final_usages));
}


//mimics Excel's percentile function
pub fn calculate_percentile(v: &[fsize], pct: fsize) -> fsize {

    // Validate input
    assert!(!v.is_empty(), "Cannot compute percentile of an empty slice");
    assert!(pct >= 0.0 && pct <= 1.0, "Percent must be between 0.0 and 1.0");

    // Create a sorted copy of the data
    let mut sorted = v.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let n = sorted.len();
    // Compute the rank using Excel's formula (1-indexed):
    // k = pct * (n - 1) + 1
    let k = pct * ((n - 1) as f64) + 1.0;

    // Determine the lower and upper indices (still 1-indexed)
    let lower_index = k.floor() as usize;
    let upper_index = k.ceil() as usize;
    let fraction = k - (lower_index as f64);

    // Convert indices to 0-indexed by subtracting 1
    let lower_value = sorted[lower_index - 1];
    let upper_value = sorted[upper_index - 1];

    // If k is an integer, fraction is 0 so this returns lower_value exactly.
    lower_value + fraction * (upper_value - lower_value)
}

pub fn calculate_median(v: &[fsize]) -> fsize {
    calculate_percentile(v, 0.5)
}

pub fn calculate_average(v: &[fsize]) -> fsize {
    v.iter().sum::<fsize>() / v.len() as fsize
}

pub fn calculate_stddev(v: &[fsize]) -> fsize {
    let avg = calculate_average(v);
    (v.iter().map(|x| (x - avg).powi(2)).sum::<fsize>() / v.len() as fsize).sqrt()
}