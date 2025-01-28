extern crate core;

use std::path::Path;
use std::time::{Duration, Instant};
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use log::warn;
use mimalloc::MiMalloc;
use once_cell::sync::Lazy;
use rand::prelude::SmallRng;
use rand::SeedableRng;
use gls_strip_packing::{io, DRAW_OPTIONS, SVG_OUTPUT_DIR};
use gls_strip_packing::io::layout_to_svg::s_layout_to_svg;
use gls_strip_packing::io::svg_util::{SvgDrawOptions, SvgLayoutTheme};
use gls_strip_packing::opt::constr_builder::ConstructiveBuilder;
use gls_strip_packing::opt::gls_optimizer::GLSOptimizer;
use gls_strip_packing::sample::search::SearchConfig;

const INPUT_FILE: &str = "../jagua-rs/assets/trousers.json";

const TIME_LIMIT_S: u64 = 20 * 60;

const N_THREADS: usize = 8;


//const RNG_SEED: Option<usize> = Some(12079827122912017592);

const RNG_SEED: Option<usize> = Some(11228681083063888015);
fn main() {

    rayon::ThreadPoolBuilder::new().num_threads(N_THREADS).build_global().unwrap();

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

    let rng = match RNG_SEED {
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

    let mut constr_builder = ConstructiveBuilder::new(sp_instance.clone(), cde_config, rng, constr_search_config);
    constr_builder.build();

    let problem = constr_builder.prob;
    let rng = constr_builder.rng;

    let mut gls_opt = GLSOptimizer::new(problem, sp_instance, rng, SVG_OUTPUT_DIR.to_string());

    let solution = gls_opt.solve(Duration::from_secs(TIME_LIMIT_S));

    io::write_svg(
        &s_layout_to_svg(&solution.layout_snapshots[0], &instance, DRAW_OPTIONS),
        Path::new(format!("{}/{}.svg", SVG_OUTPUT_DIR, "solution").as_str()),
    );
    
    println!("Hello, world!");
}
