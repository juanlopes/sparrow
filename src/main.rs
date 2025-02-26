extern crate core;

use std::ops::Add;
use gls_strip_packing::opt::constr_builder::ConstructiveBuilder;
use gls_strip_packing::opt::gls_orchestrator::GLSOrchestrator;
use gls_strip_packing::sample::search::SearchConfig;
use gls_strip_packing::util::io;
use gls_strip_packing::util::io::layout_to_svg::s_layout_to_svg;
use gls_strip_packing::{DRAW_OPTIONS, SVG_OUTPUT_DIR};
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use log::{info, warn};
use rand::SeedableRng;
use rand::prelude::SmallRng;
use std::path::Path;
use std::time::{Duration, Instant};
use ordered_float::OrderedFloat;
use gls_strip_packing::opt::post_optimizer::compact;

const INPUT_FILE: &str = "libs/jagua-rs/assets/trousers.json";

const GLS_TIME_LIMIT_S: u64 = 3 * 60;

const POST_TIME_LIMIT_S: u64 = 2 * 60;

const RNG_SEED: Option<usize> = None;

fn main() {
    if cfg!(debug_assertions) {
        io::init_logger(log::LevelFilter::Debug);
    } else {
        io::init_logger(log::LevelFilter::Info);
    }

    let json_instance = io::read_json_instance(Path::new(&INPUT_FILE));

    let cde_config = CDEConfig {
        quadtree_depth: 3,
        hpg_n_cells: 0,
        item_surrogate_config: SPSurrogateConfig {
            pole_coverage_goal: 0.95,
            max_poles: 20,
            n_ff_poles: 4,
            n_ff_piers: 0,
        },
    };

    let parser = Parser::new(PolySimplConfig::Disabled, cde_config, true);
    let instance = parser.parse(&json_instance);

    let sp_instance = match instance.clone() {
        Instance::SP(spi) => spi,
        _ => panic!("expected strip packing instance"),
    };

    info!("[MAIN] loaded instance {} with #{} items", json_instance.name, instance.total_item_qty());

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

    let constr_search_config = SearchConfig {
        n_bin_samples: 1000,
        n_focussed_samples: 0,
        n_coord_descents: 3,
    };

    let mut constr_builder = ConstructiveBuilder::new(sp_instance.clone(), cde_config, rng, constr_search_config);
    constr_builder.build();

    let mut gls_opt = GLSOrchestrator::new(constr_builder.prob, sp_instance, constr_builder.rng, SVG_OUTPUT_DIR.to_string());

    let mut solutions = gls_opt.solve(Duration::from_secs(GLS_TIME_LIMIT_S));
    let sol = solutions.last().expect("no solutions found");

    let compacted_sol = compact(&mut gls_opt, &sol, Instant::now().add(Duration::from_secs(POST_TIME_LIMIT_S)));
    println!("[POST] from {:.3}% to {:.3}% (+{:.3}%)", sol.usage * 100.0, compacted_sol.usage * 100.0, (compacted_sol.usage - sol.usage) * 100.0);

    io::write_svg(
        &s_layout_to_svg(&compacted_sol.layout_snapshots[0], &instance, DRAW_OPTIONS),
        Path::new(format!("{}/{}.svg", SVG_OUTPUT_DIR, "final").as_str()),
    );
}
