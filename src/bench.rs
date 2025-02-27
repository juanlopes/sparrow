extern crate core;

use gls_strip_packing::config::{DRAW_OPTIONS, N_WORKERS, OUTPUT_DIR, RNG_SEED};
use gls_strip_packing::opt::constr_builder::ConstructiveBuilder;
use gls_strip_packing::opt::gls_orchestrator::GLSOrchestrator;
use gls_strip_packing::opt::post_optimizer::post_optimize;
use gls_strip_packing::sample::search::SearchConfig;
use gls_strip_packing::util::io;
use gls_strip_packing::util::io::layout_to_svg::s_layout_to_svg;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::fsize;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use ordered_float::OrderedFloat;
use rand::prelude::SmallRng;
use rand::{Rng, SeedableRng};
use std::path::Path;
use std::time::{Duration, Instant};
use chrono::Local;

fn main() {
    //the input file is the first argument
    let args: Vec<String> = std::env::args().collect();
    let n_runs_total = args[1].parse::<usize>().unwrap();
    let json_instance = io::read_json_instance(Path::new(&args[2]));
    let time_limit: u64 = args[3].parse().expect("third argument must be the time limit in seconds");
    let time_limit = Duration::from_secs(time_limit);

    println!("[BENCH] git commit hash: {}", get_git_commit_hash());
    println!("[BENCH] system time: {}", Local::now());

    let mut rng = match RNG_SEED {
        Some(seed) => {
            println!("[BENCH] using provided seed: {}", seed);
            SmallRng::seed_from_u64(seed as u64)
        }
        None => {
            let seed = rand::random();
            println!("[BENCH] no seed provided, using: {}", seed);
            SmallRng::seed_from_u64(seed)
        }
    };

    let n_runs_per_iter = (num_cpus::get_physical() / N_WORKERS).min(n_runs_total);
    let n_iterations = (n_runs_total as fsize / n_runs_per_iter as fsize).ceil() as usize;

    println!(
        "[BENCH] starting bench for {} ({}x{} runs across {} cores, {:?} timelimit)",
        json_instance.name, n_iterations, n_runs_per_iter, num_cpus::get_physical(), time_limit
    );

    let cde_config = CDEConfig {
        quadtree_depth: 4,
        hpg_n_cells: 0,
        item_surrogate_config: SPSurrogateConfig {
            pole_coverage_goal: 0.95,
            max_poles: 20,
            n_ff_poles: 2,
            n_ff_piers: 0,
        },
    };

    let parser = Parser::new(PolySimplConfig::Disabled, cde_config, true);
    let instance = parser.parse(&json_instance);

    let sp_instance = match instance.clone() {
        Instance::SP(spi) => spi,
        _ => panic!("Expected SPInstance"),
    };

    let constr_search_config = SearchConfig {
        n_bin_samples: 1000,
        n_focussed_samples: 0,
        n_coord_descents: 3,
    };

    let mut final_solutions = vec![];

    for i in 0..n_iterations {
        println!("[BENCH] starting iter {}/{}", i + 1, n_iterations);
        let mut iter_solutions = vec![None; n_runs_per_iter];
        rayon::scope(|s| {
            for (j, sol_slice) in iter_solutions.iter_mut().enumerate() {
                let bench_idx = i * n_runs_per_iter + j;
                let sols_output_dir = format!("{OUTPUT_DIR}/bench_{}_sols_{}", bench_idx, json_instance.name);
                let constr_builder = ConstructiveBuilder::new(
                    sp_instance.clone(),
                    cde_config,
                    SmallRng::seed_from_u64(rng.random()),
                    constr_search_config,
                );

                s.spawn(move |_| {
                    let mut gls_opt = GLSOrchestrator::from_builder(constr_builder, sols_output_dir);
                    let solutions = gls_opt.solve(time_limit);
                    let final_gls_sol = solutions.last().expect("no solutions found");

                    let start_post = Instant::now();
                    let compacted_sol = post_optimize(&mut gls_opt, &final_gls_sol);
                    println!("[BENCH] [id:{bench_idx}] done, gls: {:.3}%, post: {:.3}% (+{:.3}%) in  {:?}ms)", final_gls_sol.usage * 100.0, compacted_sol.usage * 100.0, (compacted_sol.usage - final_gls_sol.usage) * 100.0, start_post.elapsed().as_millis());

                    *sol_slice = Some(compacted_sol);
                })
            }
        });
        final_solutions.extend(iter_solutions.into_iter().flatten());
    }

    //print statistics about the solutions, print best, worst, median and average
    let (final_widths, final_usages): (Vec<fsize>, Vec<fsize>) = final_solutions
        .iter()
        .map(|s| {
            let width = s.layout_snapshots[0].bin.bbox().width();
            let usage = s.layout_snapshots[0].usage;
            (width, usage * 100.0)
        })
        .unzip();

    let best_final_solution = final_solutions.iter().max_by_key(|s| OrderedFloat(s.usage)).unwrap();

    io::write_svg(
        &s_layout_to_svg(&best_final_solution.layout_snapshots[0], &instance, DRAW_OPTIONS),
        Path::new(format!("{OUTPUT_DIR}/final_best_{}.svg", json_instance.name).as_str()),
    );

    println!("==== BENCH FINISHED ====");

    println!("Widths:\n{:?}", &final_widths);
    println!("Usages:\n{:?}", &final_usages);

    println!("---- WIDTH STATS ----");
    println!("Worst:  {:.3}",final_widths.iter().max_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("25per:  {:.3}", calculate_percentile(&final_widths, 0.75));
    println!("Med:    {:.3}", calculate_median(&final_widths));
    println!("75per:  {:.3}", calculate_percentile(&final_widths, 0.25));
    println!("Best:   {:.3}", final_widths.iter().min_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("Avg:    {:.3}", calculate_average(&final_widths));
    println!("Stddev: {:.3}", calculate_stddev(&final_widths));
    println!("---- USAGE STATS ----");
    println!("Worst:  {:.3}", final_usages.iter().min_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("25per:  {:.3}", calculate_percentile(&final_usages, 0.25));
    println!("Median: {:.3}", calculate_median(&final_usages));
    println!("75per:  {:.3}", calculate_percentile(&final_usages, 0.75));
    println!("Best:   {:.3}", final_usages.iter().max_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("Avg:    {:.3}", calculate_average(&final_usages));
    println!("Stddev: {:.3}", calculate_stddev(&final_usages));
    println!("======================");
}

//mimics Excel's percentile function
pub fn calculate_percentile(v: &[fsize], pct: fsize) -> fsize {
    // Validate input
    assert!(!v.is_empty(), "Cannot compute percentile of an empty slice");
    assert!(
        pct >= 0.0 && pct <= 1.0,
        "Percent must be between 0.0 and 1.0"
    );

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

pub fn get_git_commit_hash() -> String {
    let output = std::process::Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .expect("Failed to execute git command");

    match output.status.success() {
        true => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        false => "unknown".to_string(),
    }
}
