extern crate core;

use clap::Parser as Clap;
use jagua_rs::entities::general::Instance;
use jagua_rs::io::parser::Parser;
use log::{info, warn, Level};
use rand::prelude::SmallRng;
use rand::SeedableRng;
use sparrow::config::*;
use sparrow::optimizer::{optimize, Terminator};
use sparrow::util::io;
use sparrow::util::io::cli::MainCli;
use sparrow::util::io::layout_to_svg::s_layout_to_svg;
use std::fs;
use std::path::Path;
use std::time::Duration;
use sparrow::util::io::to_sp_instance;

fn main() {
    fs::create_dir_all(OUTPUT_DIR).expect("could not create output directory");
    match cfg!(debug_assertions) {
        true => io::init_logger(LOG_LEVEL_FILTER_DEBUG),
        false => io::init_logger(LOG_LEVEL_FILTER_RELEASE),
    }

    let args = MainCli::parse();
    let input_file_path = &args.input;
    let (explore_dur, compress_dur) = match (args.global_time, args.exploration, args.compression) {
        (Some(gt), None, None) => {
            (Duration::from_secs(gt).mul_f32(EXPLORE_TIME_RATIO), Duration::from_secs(gt).mul_f32(COMPRESS_TIME_RATIO))
        },
        (None, Some(et), Some(ct)) => {
            (Duration::from_secs(et), Duration::from_secs(ct))
        },
        (None, None, None) => {
            warn!("[MAIN] No time limit specified");
            (Duration::from_secs(600).mul_f32(EXPLORE_TIME_RATIO), Duration::from_secs(600).mul_f32(COMPRESS_TIME_RATIO))
        },
        _ => unreachable!("invalid cli pattern (clap should have caught this)"),
    };

    info!("[MAIN] Configured to explore for {}s and compress for {}s", explore_dur.as_secs(), compress_dur.as_secs());

    let rng = match RNG_SEED {
        Some(seed) => {
            info!("[MAIN] using seed: {}", seed);
            SmallRng::seed_from_u64(seed as u64)
        },
        None => {
            let seed = rand::random();
            warn!("[MAIN] no seed provided, using: {}", seed);
            SmallRng::seed_from_u64(seed)
        }
    };

    info!("[MAIN] system time: {}", jiff::Timestamp::now());

    let json_instance = io::read_json_instance(Path::new(&input_file_path));

    let parser = Parser::new(CDE_CONFIG, SIMPL_TOLERANCE, MIN_ITEM_SEPARATION);
    let any_instance = parser.parse(&json_instance);
    let instance = to_sp_instance(any_instance.as_ref()).expect("Expected SPInstance");

    info!("[MAIN] loaded instance {} with #{} items", json_instance.name, instance.total_item_qty());

    let output_folder_path = format!("{OUTPUT_DIR}/sols_{}", json_instance.name);

    let terminator = Terminator::new_with_ctrlc_handler();

    let solution = optimize(instance.clone(), rng, output_folder_path, terminator, explore_dur, compress_dur);

    {
        let svg = s_layout_to_svg(&solution.layout_snapshot, &instance, DRAW_OPTIONS, "final");
        io::write_svg(&svg, Path::new(format!("{OUTPUT_DIR}/final_{}.svg", json_instance.name).as_str()), Level::Info);
        if cfg!(feature = "live_svg") {
            io::write_svg(&svg, Path::new(format!("{LIVE_DIR}/.live_solution.svg").as_str()), Level::Trace);
        }
    }
}
