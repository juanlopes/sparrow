extern crate core;

use chrono::Local;
use gls_strip_packing::config::{CDE_CONFIG, DRAW_OPTIONS, LOG_LEVEL_FILTER_DEBUG, LOG_LEVEL_FILTER_RELEASE, OUTPUT_DIR, RNG_SEED};
use gls_strip_packing::optimize::optimize;
use gls_strip_packing::util::io;
use gls_strip_packing::util::io::layout_to_svg::s_layout_to_svg;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use log::{info, warn, Level};
use rand::prelude::SmallRng;
use rand::SeedableRng;
use std::env::args;
use std::fs;
use std::path::Path;
use std::time::Duration;

fn main() {
    let input_file_path = args().nth(1).expect("first argument must be the input file");
    let explore_time_limit: u64 = args().nth(2).unwrap().parse()
        .expect("second argument must be the time limit for the first phase [s]");
    let explore_time_limit = Duration::from_secs(explore_time_limit);

    fs::create_dir_all(OUTPUT_DIR).expect("could not create output directory");

    match cfg!(debug_assertions) {
        true => io::init_logger(LOG_LEVEL_FILTER_DEBUG),
        false => io::init_logger(LOG_LEVEL_FILTER_RELEASE),
    }

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

    info!("[MAIN] system time: {}", Local::now());

    let json_instance = io::read_json_instance(Path::new(&input_file_path));

    let parser = Parser::new(PolySimplConfig::Disabled, CDE_CONFIG, true);
    let instance = parser.parse(&json_instance);

    let sp_instance = match instance.clone() {
        Instance::SP(spi) => spi,
        _ => panic!("expected strip packing instance"),
    };

    info!("[MAIN] loaded instance {} with #{} items", json_instance.name, instance.total_item_qty());

    let output_folder_path = format!("{OUTPUT_DIR}/sols_{}", json_instance.name);

    let solution = optimize(sp_instance, rng, output_folder_path, explore_time_limit);

    {
        let svg = s_layout_to_svg(&solution.layout_snapshots[0], &instance, DRAW_OPTIONS, "final");
        io::write_svg(&svg, Path::new(format!("{OUTPUT_DIR}/final_{}.svg", json_instance.name).as_str()), Level::Info);
        io::write_svg(&svg, Path::new(format!("{OUTPUT_DIR}/.live_solution.svg").as_str()), Level::Trace);
    }
}
