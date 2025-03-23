extern crate core;

use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::io::parser::Parser;
use log::{info, warn, Level};
use rand::prelude::SmallRng;
use rand::SeedableRng;
use sparrow::config::{CDE_CONFIG, DRAW_OPTIONS, LIVE_DIR, LOG_LEVEL_FILTER_DEBUG, LOG_LEVEL_FILTER_RELEASE, OUTPUT_DIR, RNG_SEED, SIMPLIFICATION_CONFIG};
use sparrow::optimizer::{optimize, Terminator};
use sparrow::util::io;
use sparrow::util::io::layout_to_svg::s_layout_to_svg;
use std::env::args;
use std::fs;
use std::path::Path;
use std::time::Duration;

fn main() {
    let input_file_path = args().nth(1).expect("first argument must be the input file");
    let time_limit: Duration = args().nth(2).unwrap().parse::<u64>()
        .map(|s| Duration::from_secs(s))
        .expect("second argument must be the time limit [s]");

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

    info!("[MAIN] system time: {}", jiff::Timestamp::now());

    let json_instance = io::read_json_instance(Path::new(&input_file_path));

    let parser = Parser::new(SIMPLIFICATION_CONFIG, CDE_CONFIG, true);
    let instance = match parser.parse(&json_instance){
        Instance::SP(spi) => spi,
        _ => panic!("expected strip packing instance"),
    };

    info!("[MAIN] loaded instance {} with #{} items", json_instance.name, instance.total_item_qty());

    let output_folder_path = format!("{OUTPUT_DIR}/sols_{}", json_instance.name);

    let terminator = Terminator::new_with_ctrlc_handler();

    let solution = optimize(instance.clone(), rng, output_folder_path, terminator, time_limit);

    {
        let svg = s_layout_to_svg(&solution.layout_snapshots[0], &instance, DRAW_OPTIONS, "final");
        io::write_svg(&svg, Path::new(format!("{OUTPUT_DIR}/final_{}.svg", json_instance.name).as_str()), Level::Info);
        if cfg!(feature = "live_svg") {
            io::write_svg(&svg, Path::new(format!("{LIVE_DIR}/.live_solution.svg").as_str()), Level::Trace);
        }
    }
}
