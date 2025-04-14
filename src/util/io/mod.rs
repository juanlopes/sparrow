use std::any::Any;
use log::{log, Level, LevelFilter};
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use jagua_rs::entities::general::Instance;
use jagua_rs::entities::strip_packing::SPInstance;
use svg::Document;

use crate::config::{OUTPUT_DIR};
use crate::EPOCH;
use jagua_rs::io::json_instance::JsonInstance;

pub mod layout_to_svg;
pub mod svg_export;
pub mod svg_util;
pub mod cli;

pub fn to_sp_instance(instance: &dyn Instance) -> Option<SPInstance>{
    (instance as &dyn Any).downcast_ref::<SPInstance>().cloned()
}

pub fn read_json_instance(path: &Path) -> JsonInstance {
    let file = File::open(path)
        .unwrap_or_else(|err| panic!("could not open instance file: {}, {}", path.display(), err));
    let reader = BufReader::new(file);
    serde_json::from_reader(reader)
        .unwrap_or_else(|err| panic!("could not parse instance file: {}, {}", path.display(), err))
}

pub fn init_logger(level_filter: LevelFilter) {
    //remove old log file
    let _ = fs::remove_file(format!("{}/log.txt", OUTPUT_DIR));
    fern::Dispatch::new()
        // Perform allocation-free log formatting
        .format(|out, message, record| {
            let handle = std::thread::current();
            let thread_name = handle.name().unwrap_or("-");

            let duration = EPOCH.elapsed();
            let sec = duration.as_secs() % 60;
            let min = (duration.as_secs() / 60) % 60;
            let hours = (duration.as_secs() / 60) / 60;

            let prefix = format!(
                "[{}] [{:0>2}:{:0>2}:{:0>2}] <{}>",
                record.level(),
                hours,
                min,
                sec,
                thread_name,
            );

            out.finish(format_args!("{:<25}{}", prefix, message))
        })
        // Add blanket level filter -
        .level(level_filter)
        .chain(std::io::stdout())
        .chain(fern::log_file(format!("{OUTPUT_DIR}/log.txt")).unwrap())
        .apply()
        .expect("could not initialize logger");
    log!(
        Level::Info,
        "[EPOCH]: {}",
        jiff::Timestamp::now().to_string()
    );
}


pub fn write_svg(document: &Document, path: &Path, log_lvl: Level) {
    svg::save(path, document).expect("failed to write svg file");
    log!(log_lvl,
        "[IO] svg exported to file://{}",
        fs::canonicalize(&path)
            .expect("could not canonicalize path")
            .to_str()
            .unwrap()
    );
}