use std::iter;
use std::path::Path;
use std::time::Instant;
use itertools::Itertools;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::problems::problem_generic::ProblemGeneric;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use log::{info, warn};
use mimalloc::MiMalloc;
use once_cell::sync::Lazy;
use rand::prelude::{SliceRandom, SmallRng};
use rand::{Rng, SeedableRng};
use tap::Tap;
use gls_strip_packing::{io, DRAW_OPTIONS, OUTPUT_DIR, SVG_OUTPUT_DIR};
use gls_strip_packing::io::layout_to_svg::s_layout_to_svg;
use gls_strip_packing::io::svg_util::{SvgDrawOptions, SvgLayoutTheme};
use gls_strip_packing::opt::constr_builder::ConstructiveBuilder;
use gls_strip_packing::opt::gls_optimizer::GLSOptimizer;
use gls_strip_packing::sample::eval::constructive_evaluator::ConstructiveEvaluator;
use gls_strip_packing::sample::search::SearchConfig;

const INPUT_FILE: &str = "../jagua-rs/assets/swim.json";

//const RNG_SEED: Option<usize> = Some(12079827122912017592);

const RNG_SEED: Option<usize> = None;
fn main(){

    io::init_logger(log::LevelFilter::Debug);

    let num_cpus = num_cpus::get();
    info!("Number of CPUs: {}", num_cpus);

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

    let mut rng = match RNG_SEED {
        Some(seed) => SmallRng::seed_from_u64(seed as u64),
        None => {
            let seed = rand::random();
            warn!("No seed provided, using: {}", seed);
            SmallRng::seed_from_u64(seed)
        }
    };

    let problem= SPProblem::new(sp_instance.clone(), 5600.0, cde_config);

    let constr_search_config = SearchConfig{
        n_bin_samples: 1000,
        n_valid_cutoff: Some(200),
        n_focussed_samples: 0,
        n_coord_descents: 3,
    };

    let mut constr_builder = ConstructiveBuilder::new(sp_instance, cde_config, rng, constr_search_config);
    let solution = constr_builder.build();

    // let search_config = SearchConfig{n_bin_samples: 10000, n_valid_cutoff: Some(200), n_focussed_samples: 0, n_coord_descents: 10};
    // // let item_id_order = instance.items().iter()
    // //     .map(|(item, qty)| iter::repeat(item.id).take(*qty))
    // //     .flatten()
    // //     .collect_vec()
    // //     .tap_mut(|v| v.shuffle(&mut rng));
    //
    // let mut opt = ConstructiveBuilder::new(problem, sp_instance, rng, search_config, SVG_OUTPUT_DIR.to_string());
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

    // for i in 0..1 {
    //     let rng = SmallRng::from_seed(rng.gen());
    //     let search_config = SearchConfig{n_bin_samples: 200, n_valid_cutoff: None, n_focussed_samples: 0, n_coord_descents: 2};
    //     let mut broad_opt = BroadOptimizer::new(num_cpus, problem.clone(), sp_instance.clone(), rng.clone(), search_config, format!("{}/{}_{i}", SVG_OUTPUT_DIR, "broad"));
    //     let best_broad_solution = broad_opt.optimize();
    //
    //     io::write_svg(
    //         &s_layout_to_svg(&best_broad_solution.layout_snapshots[0], &instance, DRAW_OPTIONS),
    //         Path::new(format!("{}/{}_{i}.svg", OUTPUT_DIR, "solution").as_str()),
    //     );
    //
    //     let mut gls_problem= problem.clone();
    //     gls_problem.restore_to_solution(&best_broad_solution);
    //
    //     let mut gls_opt = GLSOptimizer::new(gls_problem, sp_instance.clone(), rng, SVG_OUTPUT_DIR.to_string());
    //     gls_opt.change_strip_width(6000.0);
    //
    //     let solution = gls_opt.solve();
    // }

    io::write_svg(
         &s_layout_to_svg(&solution.layout_snapshots[0], &instance, DRAW_OPTIONS),
         Path::new(format!("{}/{}.svg", SVG_OUTPUT_DIR, "solution").as_str()),
    );
}