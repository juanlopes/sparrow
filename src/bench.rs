extern crate core;

use sparrow::config::{CDE_CONFIG, LBF_SAMPLE_CONFIG, DRAW_OPTIONS, OUTPUT_DIR, RNG_SEED, SEPARATOR_CONFIG_COMPRESS, SEP_CONFIG_EXPLORE, SIMPLIFICATION_CONFIG};
use sparrow::optimizer::lbf::LBFBuilder;
use sparrow::optimizer::separator::Separator;
use sparrow::optimizer::{compress, explore, Terminator};
use sparrow::util::io;
use sparrow::util::io::layout_to_svg::s_layout_to_svg;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::io::parser::Parser;
use ordered_float::OrderedFloat;
use rand::prelude::SmallRng;
use rand::{Rng, SeedableRng};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

fn main() {
    //the input file is the first argument
    let input_file_path = std::env::args().nth(1).expect("first argument must be the input file");
    let explore_time_limit: u64 = std::env::args().nth(2).unwrap().parse()
        .expect("second argument must be the time limit in seconds");
    let explore_time_limit = Duration::from_secs(explore_time_limit);
    let n_runs_total = std::env::args().nth(3).expect("third argument must be the number of runs")
        .parse().expect("third argument must be the number of runs");

    fs::create_dir_all(OUTPUT_DIR).expect("could not create output directory");

    let json_instance = io::read_json_instance(Path::new(&input_file_path));

    println!("[BENCH] git commit hash: {}", get_git_commit_hash());
    println!("[BENCH] system time: {}", jiff::Timestamp::now());

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

    let n_runs_per_iter = (num_cpus::get_physical() / SEP_CONFIG_EXPLORE.n_workers).min(n_runs_total);
    let n_batches = (n_runs_total as f32 / n_runs_per_iter as f32).ceil() as usize;

    println!(
        "[BENCH] starting bench for {} ({}x{} runs across {} cores, {:?} explore timelimit)",
        json_instance.name, n_batches, n_runs_per_iter, num_cpus::get_physical(), explore_time_limit
    );

    let parser = Parser::new(SIMPLIFICATION_CONFIG, CDE_CONFIG, true);
    let instance = parser.parse(&json_instance);

    let instance = match instance {
        Instance::SP(spi) => spi,
        _ => panic!("Expected SPInstance"),
    };

    let mut final_solutions = vec![];

    let dummy_terminator = Terminator::dummy();

    for i in 0..n_batches {
        println!("[BENCH] batch {}/{}", i + 1, n_batches);
        let mut iter_solutions = vec![None; n_runs_per_iter];
        rayon::scope(|s| {
            for (j, sol_slice) in iter_solutions.iter_mut().enumerate() {
                let bench_idx = i * n_runs_per_iter + j;
                let output_folder_path = format!("{OUTPUT_DIR}/bench_{}_sols_{}", bench_idx, json_instance.name);
                let instance = instance.clone();
                let rng = SmallRng::seed_from_u64(rng.random());
                let mut terminator = dummy_terminator.clone();

                s.spawn(move |_| {
                    let builder = LBFBuilder::new(instance.clone(), CDE_CONFIG, rng, LBF_SAMPLE_CONFIG).construct();
                    let mut expl_separator = Separator::new(builder.instance, builder.prob, builder.rng, output_folder_path, 0, SEP_CONFIG_EXPLORE);

                    terminator.set_timeout(explore_time_limit);
                    let solutions = explore(&mut expl_separator, &terminator);
                    let final_explore_sol = solutions.last().expect("no solutions found during exploration");

                    let start_comp = Instant::now();

                    terminator.clear_timeout().reset_ctrlc();
                    let mut cmpr_separator = Separator::new(expl_separator.instance, expl_separator.prob, expl_separator.rng, expl_separator.output_svg_folder, expl_separator.svg_counter, SEPARATOR_CONFIG_COMPRESS);
                    let final_sol = compress(&mut cmpr_separator, final_explore_sol, &terminator);

                    println!("[BENCH] [id:{:>3}] finished, expl: {:.3}% ({}s), cmpr: {:.3}% (+{:.3}%) ({}s)",
                             bench_idx,
                             final_explore_sol.usage * 100.0, explore_time_limit.as_secs(),
                             final_sol.usage * 100.0,
                             final_sol.usage * 100.0 - final_explore_sol.usage * 100.0,
                             start_comp.elapsed().as_secs()
                    );

                    io::write_svg(
                        &s_layout_to_svg(&final_sol.layout_snapshots[0], &instance, DRAW_OPTIONS, &*format!("final_bench_{}", bench_idx)),
                        Path::new(&format!("{OUTPUT_DIR}/final_bench_{}.svg", bench_idx)),
                        log::Level::Info,
                    );

                    *sol_slice = Some(final_sol);
                })
            }
        });
        final_solutions.extend(iter_solutions.into_iter().flatten());
    }

    //print statistics about the solutions, print best, worst, median and average
    let (final_widths, final_usages): (Vec<f32>, Vec<f32>) = final_solutions
        .iter()
        .map(|s| {
            let width = s.layout_snapshots[0].bin.bbox().width();
            let usage = s.layout_snapshots[0].usage;
            (width, usage * 100.0)
        })
        .unzip();

    let best_final_solution = final_solutions.iter().max_by_key(|s| OrderedFloat(s.usage)).unwrap();

    io::write_svg(
        &s_layout_to_svg(&best_final_solution.layout_snapshots[0], &instance, DRAW_OPTIONS, "final_best"),
        Path::new(format!("{OUTPUT_DIR}/final_best_{}.svg", json_instance.name).as_str()),
        log::Level::Info,
    );

    println!("==== BENCH FINISHED ====");

    println!("widths:\n{:?}", &final_widths);
    println!("usages:\n{:?}", &final_usages);

    println!("---- WIDTH STATS ----");
    println!("worst:  {:.3}", final_widths.iter().max_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("25%:    {:.3}", calculate_percentile(&final_widths, 0.75));
    println!("med:    {:.3}", calculate_median(&final_widths));
    println!("75%:    {:.3}", calculate_percentile(&final_widths, 0.25));
    println!("best:   {:.3}", final_widths.iter().min_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("avg:    {:.3}", calculate_average(&final_widths));
    println!("stddev: {:.3}", calculate_stddev(&final_widths));
    println!("---- USAGE STATS ----");
    println!("worst:  {:.3}", final_usages.iter().min_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("25%:    {:.3}", calculate_percentile(&final_usages, 0.25));
    println!("median: {:.3}", calculate_median(&final_usages));
    println!("75%:    {:.3}", calculate_percentile(&final_usages, 0.75));
    println!("best:   {:.3}", final_usages.iter().max_by_key(|&x| OrderedFloat(*x)).unwrap());
    println!("avg:    {:.3}", calculate_average(&final_usages));
    println!("stddev: {:.3}", calculate_stddev(&final_usages));
    println!("======================");
}

//mimics Excel's percentile function
pub fn calculate_percentile(v: &[f32], pct: f32) -> f32 {
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
    let k = pct * (n - 1) as f32 + 1.0;

    // Determine the lower and upper indices (still 1-indexed)
    let lower_index = k.floor() as usize;
    let upper_index = k.ceil() as usize;
    let fraction = k - (lower_index as f32);

    // Convert indices to 0-indexed by subtracting 1
    let lower_value = sorted[lower_index - 1];
    let upper_value = sorted[upper_index - 1];

    // If k is an integer, fraction is 0 so this returns lower_value exactly.
    lower_value + fraction * (upper_value - lower_value)
}

pub fn calculate_median(v: &[f32]) -> f32 {
    calculate_percentile(v, 0.5)
}

pub fn calculate_average(v: &[f32]) -> f32 {
    v.iter().sum::<f32>() / v.len() as f32
}

pub fn calculate_stddev(v: &[f32]) -> f32 {
    let avg = calculate_average(v);
    (v.iter().map(|x| (x - avg).powi(2)).sum::<f32>() / v.len() as f32).sqrt()
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
