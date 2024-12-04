use std::path::Path;
use std::time::Instant;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::problems::problem_generic::ProblemGeneric;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use log::warn;
use mimalloc::MiMalloc;
use once_cell::sync::Lazy;
use rand::prelude::SmallRng;
use rand::{Rng, SeedableRng};
use gls_strip_packing::{io, DRAW_OPTIONS, OUTPUT_DIR, SVG_OUTPUT_DIR};
use gls_strip_packing::io::layout_to_svg::s_layout_to_svg;
use gls_strip_packing::io::svg_util::{SvgDrawOptions, SvgLayoutTheme};
use gls_strip_packing::opt::broad_optimizer::BroadOptimizer;
use gls_strip_packing::opt::constr_builder;
use gls_strip_packing::opt::constr_builder::ConstructiveBuilder;

const INPUT_FILE: &str = "../jagua-rs/assets/swim.json";

//const RNG_SEED: Option<usize> = Some(12079827122912017592);

const RNG_SEED: Option<usize> = Some(0);
fn main(){

    io::init_logger(log::LevelFilter::Info);

    let json_instance = io::read_json_instance(Path::new(&INPUT_FILE));

    let cde_config = CDEConfig{
        quadtree_depth: 4,
        hpg_n_cells: 2000,
        item_surrogate_config: SPSurrogateConfig {
            pole_coverage_goal: 0.95,
            max_poles: 20,
            n_ff_poles: 4,
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

    let problem= SPProblem::new(sp_instance.clone(), 5600.0, cde_config);

    // let mut opt = ConstructiveBuilder::new(problem, sp_instance, rng);
    // let start = Instant::now();
    // let p_opts = opt.fill();
    // let solution = opt.prob.create_solution(None);
    // dbg!(constr_builder::value_solution(&solution));
    // let elapsed = start.elapsed();
    // println!("Elapsed: {:?}", elapsed);
    //
    // io::write_svg(
    //     &s_layout_to_svg(&solution.layout_snapshots[0], &instance, DRAW_OPTIONS),
    //     Path::new(format!("{}/{}.svg", SVG_OUTPUT_DIR, "solution").as_str()),
    // );

    for i in 0..10 {
        let rng = SmallRng::from_seed(rng.gen());
        let mut broad_opt = BroadOptimizer::new(16, problem.clone(), sp_instance.clone(), rng);
        broad_opt.optimize();
        let solution = broad_opt.master_prob.create_solution(None);

        io::write_svg(
            &s_layout_to_svg(&solution.layout_snapshots[0], &instance, DRAW_OPTIONS),
            Path::new(format!("{}/{}_{i}.svg", OUTPUT_DIR, "solution").as_str()),
        );
    }
}