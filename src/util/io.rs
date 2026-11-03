use crate::EPOCH;
use crate::config::SparrowConfig;
use anyhow::{bail, Context, Result};
use clap::Parser;
use jagua_rs::io::ext_repr::ExtShape;
use jagua_rs::probs::spp::io::ext_repr::{ExtSPInstance, ExtSPSolution};
use log::{log, warn, Level, LevelFilter};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::fs::File;
use std::num::NonZeroUsize;
use std::path::Path;
use svg::Document;

/// Real DPI used when `--dpi` is omitted.
///
/// `units = mm * dpi / 25.4`. At 96 DPI that is `mm * 96 / 25.4` pixels.
pub const DEFAULT_DPI: f32 = 96.0;

/// Historical default of `--dpi`.
///
/// This number is already pixels per millimeter (`96 / 25.4`), not a DPI.
/// The old formula divided by 25.4 again, so 3 mm became ~0.4464 instead of ~11.34.
pub const LEGACY_DPI_SPEC: &str = "3.7795275591";

#[derive(Parser)]
pub struct MainCli {
    /// Path to input file (mandatory)
    #[arg(short = 'i', long, help = "Path to the input JSON file, or a solution JSON file for warm starting")]
    pub input: String,

    /// Global time limit in seconds (mutually exclusive with -e and -c)
    #[arg(short = 't', long, conflicts_with_all = &["exploration", "compression"], help = "Set a global time limit (in seconds)")]
    pub global_time: Option<u64>,

    /// Exploration time limit in seconds (requires compression time)
    #[arg(short = 'e', long, requires = "compression", help = "Set the exploration phase time limit (in seconds)")]
    pub exploration: Option<u64>,

    /// Compression time limit in seconds (requires exploration time)
    #[arg(short = 'c', long, requires = "exploration", help = "Set the compression phase time limit (in seconds)")]
    pub compression: Option<u64>,

    /// Enable early and automatic termination
    #[arg(short = 'x', long, help = "Enable early termination of the optimization process")]
    pub early_termination: bool,

    /// Minimum distance between items in millimeters.
    ///
    /// Overrides `min_item_separation` from the JSON only when this flag is present.
    #[arg(short = 'm', long, help = "Set minimum distance between items in millimeters")]
    pub min_separation_mm: Option<f32>,

    /// Dots per inch used to convert `--min-separation-mm` into input units.
    ///
    /// Omitted means 96. `SPARROW_LEGACY_DPI=1` restores the historical default
    /// `3.7795275591` (pixels/mm passed through the same `/ 25.4` formula).
    #[arg(long, help = "Dots per inch for converting millimeters to input units (default: 96)")]
    pub dpi: Option<f32>,

    #[arg(short = 's', long, help = "Fixed seed for the random number generator")]
    pub rng_seed: Option<u64>,

    /// Number of worker threads used by the separator
    #[arg(long)]
    pub workers: Option<NonZeroUsize>,
}

impl MainCli {
    /// Applies optional CLI values that override the default optimizer configuration.
    pub fn apply_config_overrides(&self, config: &mut SparrowConfig) {
        if let Some(rng_seed) = self.rng_seed {
            warn!("[MAIN] overriding RNG seed: {rng_seed}");
            config.rng_seed = Some(rng_seed as usize);
        }
        if let Some(workers) = self.workers {
            warn!("[MAIN] overriding separator worker count: {workers}");
            config.expl_cfg.separator_config.n_workers = workers.get();
            config.cmpr_cfg.separator_config.n_workers = workers.get();
        }
    }
}

/// Instance loaded for the solver, plus the JSON shape to write back.
pub struct SppInput {
    pub instance: ExtSPInstance,
    /// Warm-start solution, if the file contained one.
    ///
    /// Rotations are in degrees, which is what `jagua-rs` import expects.
    /// Files produced by this fork store radians; they are converted on read.
    pub solution: Option<ExtSPSolution>,
    echo: InstanceEcho,
}

enum InstanceEcho {
    /// Pre-0.3.0 instance: `allowed_orientations`, no required `orientation`.
    Legacy(LegacySpInstance),
    /// Upstream 0.3.0 instance.
    Current(ExtSPInstance),
}

impl SppInput {
    /// Sets the separation used by the solver.
    ///
    /// Legacy output gains `min_item_separation` only when the input JSON already
    /// had that key, matching the historical file shape. Current-format output
    /// always records the value that was solved.
    pub fn apply_min_separation(&mut self, units: f32) {
        self.instance.min_item_separation = units;
        match &mut self.echo {
            InstanceEcho::Legacy(legacy) if legacy.min_item_separation.is_some() => {
                legacy.min_item_separation = Some(units);
            }
            InstanceEcho::Current(current) => {
                current.min_item_separation = units;
            }
            InstanceEcho::Legacy(_) => {}
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct LegacySpInstance {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_item_separation: Option<f32>,
    items: Vec<LegacyItem>,
    strip_height: f32,
}

#[derive(Serialize, Deserialize, Clone)]
struct LegacyItem {
    id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    allowed_orientations: Option<Vec<f32>>,
    shape: ExtShape,
    min_quality: Option<usize>,
    demand: u64,
}

#[derive(Serialize)]
struct LegacySolution {
    strip_width: f32,
    layout: LegacyLayout,
    density: f32,
    run_time_sec: u64,
}

#[derive(Serialize)]
struct LegacyLayout {
    container_id: u64,
    placed_items: Vec<LegacyPlacedItem>,
    density: f32,
}

#[derive(Serialize)]
struct LegacyPlacedItem {
    item_id: u64,
    transformation: LegacyTransformation,
}

/// Historical solution transform: rotation in radians, then translation.
/// `reflected` is written only when a placement actually used reflection.
#[derive(Serialize)]
struct LegacyTransformation {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    reflected: bool,
    rotation: f32,
    translation: (f32, f32),
}

pub fn init_logger(level_filter: LevelFilter, log_file_path: &Path) -> Result<()> {
    //remove old log file
    let _ = fs::remove_file(log_file_path);
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
        .chain(fern::log_file(log_file_path)?)
        .apply()?;
    log!(
        Level::Info,
        "[EPOCH]: {}",
        jiff::Timestamp::now()
    );
    Ok(())
}


pub fn write_svg(document: &Document, path: &Path, log_lvl: Level) -> Result<()> {
    //make sure the parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("could not create parent directory for svg file")?;
    }
    svg::save(path, document)?;
    log!(log_lvl,
        "[IO] svg exported to file://{}",
        fs::canonicalize(path)
            .expect("could not canonicalize path")
            .to_str()
            .unwrap()
    );
    Ok(())
}

pub fn write_json(json: &impl Serialize, path: &Path, log_lvl: Level) -> Result<()> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, json)?;
    log!(log_lvl,
        "[IO] json exported to file://{}",
        fs::canonicalize(path)
            .expect("could not canonicalize path")
            .to_str()
            .unwrap()
    );
    Ok(())
}

pub fn read_spp_input(path: &Path) -> Result<SppInput> {
    let input_str = fs::read_to_string(path).context("could not read input file")?;
    parse_spp_input(&input_str)
}

/// Parses either a legacy instance (`allowed_orientations`) or a 0.3.0 instance
/// (`orientation.rotation`). A trailing `solution` object, if present, is kept
/// for warm start. Solution rotations in the file are radians.
pub fn parse_spp_input(input_str: &str) -> Result<SppInput> {
    let mut root: Value = serde_json::from_str(input_str).context("input is not valid JSON")?;
    let solution_value = match root.as_object_mut().and_then(|map| map.remove("solution")) {
        Some(Value::Null) | None => None,
        Some(solution) => Some(solution),
    };

    let legacy = is_legacy_instance(&root)?;
    let (instance, echo) = if legacy {
        let legacy_inst: LegacySpInstance = serde_json::from_value(root.clone())
            .context("not a valid strip packing instance")?;
        let instance = legacy_json_to_current(root)?;
        (instance, InstanceEcho::Legacy(legacy_inst))
    } else {
        let instance: ExtSPInstance = serde_json::from_value(root)
            .context("not a valid strip packing instance")?;
        (instance.clone(), InstanceEcho::Current(instance))
    };

    let solution = match solution_value {
        Some(mut solution) => {
            // This fork writes radians. jagua-rs 1.x imports degrees.
            scale_solution_rotations(&mut solution, true)
                .context("could not parse solution from input file")?;
            Some(
                serde_json::from_value(solution)
                    .context("could not parse solution from input file")?,
            )
        }
        None => None,
    };

    Ok(SppInput {
        instance,
        solution,
        echo,
    })
}

/// JSON document written to `output/final_{name}.json`.
///
/// The instance section keeps the shape of the input (legacy or current).
/// The solution section always uses the historical schema: `rotation` in radians,
/// and no `reflected` key unless the placement was reflected.
pub fn fork_output_value(input: &SppInput, solution: &ExtSPSolution) -> Result<Value> {
    let mut root = match &input.echo {
        InstanceEcho::Legacy(legacy) => serde_json::to_value(legacy)?,
        InstanceEcho::Current(current) => serde_json::to_value(current)?,
    };
    root["solution"] = serde_json::to_value(legacy_solution(solution))?;
    Ok(root)
}

fn legacy_solution(solution: &ExtSPSolution) -> LegacySolution {
    LegacySolution {
        strip_width: solution.strip_width,
        layout: LegacyLayout {
            container_id: solution.layout.container_id,
            placed_items: solution
                .layout
                .placed_items
                .iter()
                .map(|placed| LegacyPlacedItem {
                    item_id: placed.item_id,
                    transformation: LegacyTransformation {
                        reflected: placed.transformation.reflected,
                        rotation: placed.transformation.rotation.to_radians(),
                        translation: placed.transformation.translation,
                    },
                })
                .collect(),
            density: solution.layout.density,
        },
        density: solution.density,
        run_time_sec: solution.run_time_sec,
    }
}

fn is_legacy_instance(root: &Value) -> Result<bool> {
    let Some(items) = root.get("items").and_then(Value::as_array) else {
        bail!("instance is missing items");
    };
    if items.is_empty() {
        return Ok(true);
    }
    let mut legacy = false;
    let mut current = false;
    for item in items {
        let Some(object) = item.as_object() else {
            bail!("item must be a JSON object");
        };
        if object.contains_key("orientation") {
            current = true;
        } else {
            legacy = true;
        }
    }
    if legacy && current {
        bail!("mixed legacy allowed_orientations items and orientation items are not supported");
    }
    Ok(legacy)
}

fn legacy_json_to_current(mut root: Value) -> Result<ExtSPInstance> {
    let Some(items) = root.get_mut("items").and_then(Value::as_array_mut) else {
        bail!("instance is missing items");
    };
    for item in items {
        rewrite_legacy_item(item)?;
    }
    serde_json::from_value(root).context("not a valid strip packing instance")
}

/// Maps the pre-0.3.0 `allowed_orientations` field onto `orientation.rotation`.
///
/// Missing or null meant continuous rotation. An empty list, or a single `0`,
/// meant the part could not rotate (`RotationRange::None`), which is
/// `{"mode":"discrete","angles":[0]}` now. Any other list is that discrete set.
/// Unknown item metadata such as `dxf` is left in place; jagua-rs ignores it.
fn rewrite_legacy_item(item: &mut Value) -> Result<()> {
    let Some(object) = item.as_object_mut() else {
        bail!("item must be a JSON object");
    };
    if object.contains_key("orientation") {
        bail!("legacy item also sets orientation");
    }
    let allowed = object.remove("allowed_orientations");
    // The old parser ignored these names. The 1.x parser rejects them.
    object.remove("allowed_rotations");
    object.remove("allowed_reflection_axes");

    let rotation = match allowed {
        None | Some(Value::Null) => json!({"mode": "continuous"}),
        Some(Value::Array(angles)) if angles.is_empty() => {
            json!({"mode": "discrete", "angles": [0.0]})
        }
        Some(Value::Array(angles)) if angles.len() == 1 && angle_is_zero(&angles[0]) => {
            json!({"mode": "discrete", "angles": [0.0]})
        }
        Some(Value::Array(angles)) => json!({"mode": "discrete", "angles": angles}),
        Some(other) => bail!("allowed_orientations must be an array or null, got {other}"),
    };
    object.insert(
        "orientation".to_string(),
        json!({"rotation": rotation}),
    );
    Ok(())
}

fn angle_is_zero(value: &Value) -> bool {
    value.as_f64().is_some_and(|angle| angle == 0.0)
}

fn scale_solution_rotations(solution: &mut Value, to_degrees: bool) -> Result<()> {
    let Some(items) = solution
        .pointer_mut("/layout/placed_items")
        .and_then(Value::as_array_mut)
    else {
        bail!("solution is missing layout.placed_items");
    };
    for item in items {
        let rotation = item
            .pointer("/transformation/rotation")
            .and_then(Value::as_f64)
            .context("placed item is missing transformation.rotation")?;
        let converted = if to_degrees {
            rotation.to_degrees()
        } else {
            rotation.to_radians()
        };
        let Some(slot) = item.pointer_mut("/transformation/rotation") else {
            bail!("placed item is missing transformation.rotation");
        };
        *slot = Value::from(converted);
    }
    Ok(())
}

/// `true` when `SPARROW_LEGACY_DPI` asks for the historical default.
pub fn legacy_dpi_requested() -> bool {
    std::env::var("SPARROW_LEGACY_DPI")
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

pub fn legacy_dpi() -> f32 {
    LEGACY_DPI_SPEC.parse::<f32>().expect("legacy dpi literal")
}

/// DPI that `--min-separation-mm` is converted with.
///
/// An explicit `--dpi` is always a real DPI. When the flag is omitted, the
/// default is 96 unless `legacy_default` is set, in which case the historical
/// value `3.7795275591` is used with the same formula.
pub fn effective_dpi(cli_dpi: Option<f32>, legacy_default: bool) -> f32 {
    cli_dpi.unwrap_or(if legacy_default {
        legacy_dpi()
    } else {
        DEFAULT_DPI
    })
}

/// Converts millimeters to input units.
///
/// `dpi` is pixels per inch. `units = mm * dpi / 25.4`.
pub fn mm_to_internal_units(mm: f32, dpi: f32) -> f32 {
    mm * dpi / 25.4
}
