extern crate core;

use clap::Parser as Clap;
use log::{info, warn, Level};
use rand::prelude::SmallRng;
use rand::SeedableRng;
use sparrow::config::*;
use sparrow::optimizer::optimize;
use sparrow::util::io;
use sparrow::util::io::{MainCli, SPOutput};
use std::fs;
use std::path::Path;
use std::time::Duration;
use jagua_rs::io::import::Importer;
use sparrow::EPOCH;

use anyhow::{bail, Result};
use sparrow::consts::{DEFAULT_COMPRESS_TIME_RATIO, DEFAULT_EXPLORE_TIME_RATIO, DEFAULT_FAIL_DECAY_RATIO_CMPR, DEFAULT_MAX_CONSEQ_FAILS_EXPL, LOG_LEVEL_FILTER_DEBUG, LOG_LEVEL_FILTER_RELEASE};
use sparrow::util::svg_exporter::SvgExporter;
use sparrow::util::ctrlc_terminator::CtrlCTerminator;

pub const OUTPUT_DIR: &str = "output";

pub const LIVE_DIR: &str = "data/live";

fn main() -> Result<()>{
    let mut config = DEFAULT_SPARROW_CONFIG;

    fs::create_dir_all(OUTPUT_DIR)?;
    let log_file_path = format!("{}/log.txt", OUTPUT_DIR);
    match cfg!(debug_assertions) {
        true => io::init_logger(LOG_LEVEL_FILTER_DEBUG, Path::new(&log_file_path))?,
        false => io::init_logger(LOG_LEVEL_FILTER_RELEASE, Path::new(&log_file_path))?,
    }

    let args = MainCli::parse();
    let input_file_path = &args.input;
    let (explore_dur, compress_dur) = match (args.global_time, args.exploration, args.compression) {
        (Some(gt), None, None) => {
            (Duration::from_secs(gt).mul_f32(DEFAULT_EXPLORE_TIME_RATIO), Duration::from_secs(gt).mul_f32(DEFAULT_COMPRESS_TIME_RATIO))
        },
        (None, Some(et), Some(ct)) => {
            (Duration::from_secs(et), Duration::from_secs(ct))
        },
        (None, None, None) => {
            warn!("[MAIN] no time limit specified");
            (Duration::from_secs(600).mul_f32(DEFAULT_EXPLORE_TIME_RATIO), Duration::from_secs(600).mul_f32(DEFAULT_COMPRESS_TIME_RATIO))
        },
        _ => bail!("invalid cli pattern (clap should have caught this)"),
    };
    if args.early_termination {
        config.expl_cfg.max_conseq_failed_attempts = Some(DEFAULT_MAX_CONSEQ_FAILS_EXPL);
        config.cmpr_cfg.shrink_decay = ShrinkDecayStrategy::FailureBased(DEFAULT_FAIL_DECAY_RATIO_CMPR);
        warn!("[MAIN] early termination enabled!");
    }
    if let Some(arg_rng_seed) = args.rng_seed {
        config.rng_seed = Some(arg_rng_seed as usize);
    }
    
    // Configure the time limits in the config structures
    config.expl_cfg.time_limit = explore_dur;
    config.cmpr_cfg.time_limit = compress_dur;
    
    // Configure minimum separation if provided in millimeters
    if let Some(mm_separation) = args.min_separation_mm {
        let internal_units = io::mm_to_internal_units(mm_separation, args.dpi);
        config.min_item_separation = Some(internal_units);
        info!("[MAIN] minimum separation: {:.3}mm = {:.6} internal units (DPI: {:.6})", 
              mm_separation, internal_units, args.dpi);
    }

    info!("[MAIN] configured to explore for {}s and compress for {}s", explore_dur.as_secs(), compress_dur.as_secs());

    let rng = match config.rng_seed {
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

    let ext_instance = io::read_spp_instance_json(Path::new(&input_file_path))?;

    let importer = Importer::new(config.cde_config, config.poly_simpl_tolerance, config.min_item_separation);
    let instance = jagua_rs::probs::spp::io::import(&importer, &ext_instance)?;

    info!("[MAIN] loaded instance {} with #{} items", ext_instance.name, instance.total_item_qty());
    
    let mut svg_exporter = {
        let final_svg_path = Some(format!("{OUTPUT_DIR}/final_{}.svg", ext_instance.name));

        let intermediate_svg_dir = match cfg!(feature = "only_final_svg") {
            true => None,
            false => Some(format!("{OUTPUT_DIR}/sols_{}", ext_instance.name))
        };

        let live_svg_path = match cfg!(feature = "live_svg") {
            true => Some(format!("{LIVE_DIR}/.live_solution.svg")),
            false => None
        };
        
        SvgExporter::new(
            final_svg_path,
            intermediate_svg_dir,
            live_svg_path
        )
    };
    
    let mut ctrlc_terminator = CtrlCTerminator::new();

    let solution = optimize(instance.clone(), rng, &mut svg_exporter, &mut ctrlc_terminator, &config.expl_cfg, &config.cmpr_cfg);

    let json_path = format!("{OUTPUT_DIR}/final_{}.json", ext_instance.name);
    let json_output = SPOutput {
        instance: ext_instance,
        solution: jagua_rs::probs::spp::io::export(&instance, &solution, *EPOCH)
    };
    io::write_json(&json_output, Path::new(json_path.as_str()), Level::Info)?;

    Ok(())
}
