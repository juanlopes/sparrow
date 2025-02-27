extern crate core;

use std::env::args;
use gls_strip_packing::opt::constr_builder::ConstructiveBuilder;
use gls_strip_packing::opt::gls_orchestrator::GLSOrchestrator;
use gls_strip_packing::util::io;
use gls_strip_packing::util::io::layout_to_svg::s_layout_to_svg;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::CDEConfig;
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use log::{info, warn, Level};
use rand::SeedableRng;
use rand::prelude::SmallRng;
use std::path::Path;
use std::time::{Duration, Instant};
use chrono::Local;
use ordered_float::OrderedFloat;
use gls_strip_packing::config::{CDE_CONFIG, CONSTR_SEARCH_CONFIG, DRAW_OPTIONS, LOG_LEVEL_DEBUG, LOG_LEVEL_RELEASE, OUTPUT_DIR, RNG_SEED};
use gls_strip_packing::opt::post_optimizer::post_optimize;

fn main() {
    let input_file_path = args().nth(1).expect("first argument must be the input file");
    let time_limit: u64 = args().nth(2).unwrap().parse()
        .expect("second argument must be the time limit in seconds");
    let time_limit = Duration::from_secs(time_limit);

    match cfg!(debug_assertions) {
        true => io::init_logger(LOG_LEVEL_DEBUG),
        false => io::init_logger(LOG_LEVEL_RELEASE),
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

    let constr_builder = ConstructiveBuilder::new(sp_instance.clone(), CDE_CONFIG, rng, CONSTR_SEARCH_CONFIG);

    let mut gls_opt = GLSOrchestrator::from_builder(constr_builder, format!("{OUTPUT_DIR}/sols_{}",json_instance.name));

    let solutions = gls_opt.solve(time_limit);
    let final_gls_sol = solutions.last().expect("no solutions found");

    gls_opt.log_level = Level::Debug; //switch to debug level for the final phase
    let compacted_sol = post_optimize(&mut gls_opt, &final_gls_sol);

    io::write_svg(
        &s_layout_to_svg(&compacted_sol.layout_snapshots[0], &instance, DRAW_OPTIONS, "final"),
        Path::new(format!("{OUTPUT_DIR}/final_{}.svg", json_instance.name).as_str()),
        Level::Info,
    );
}
