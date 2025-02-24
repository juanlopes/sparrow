use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use log::{Level, LevelFilter, info, log};
use svg::Document;

use jagua_rs::io::json_instance::JsonInstance;

use crate::{EPOCH, OUTPUT_DIR};
pub mod layout_to_svg;
pub mod svg_export;
pub mod svg_util;

pub fn read_json_instance(path: &Path) -> JsonInstance {
    let file = File::open(path)
        .unwrap_or_else(|err| panic!("could not open instance file: {}, {}", path.display(), err));
    let reader = BufReader::new(file);
    serde_json::from_reader(reader)
        .unwrap_or_else(|err| panic!("could not parse instance file: {}, {}", path.display(), err))
}

pub fn write_svg(document: &Document, path: &Path) {
    svg::save(path, document).expect("failed to write svg file");
    info!(
        "Solution SVG written to file://{}",
        fs::canonicalize(&path)
            .expect("could not canonicalize path")
            .to_str()
            .unwrap()
    );
}

pub fn init_logger(level_filter: LevelFilter) {
    //make the output directory if it does not exist
    fs::create_dir_all(OUTPUT_DIR).expect("could not create output directory");

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

            out.finish(format_args!("{:<27}{}", prefix, message))
        })
        // Add blanket level filter -
        .level(level_filter)
        .chain(std::io::stdout())
        .chain(fern::log_file(format!("{OUTPUT_DIR}/log.txt")).unwrap())
        .apply()
        .expect("could not initialize logger");
    log!(
        Level::Info,
        "Epoch: {}",
        humantime::format_rfc3339_seconds(std::time::SystemTime::now())
    );
}
