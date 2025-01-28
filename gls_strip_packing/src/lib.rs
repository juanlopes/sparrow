use std::time::Instant;
use mimalloc::MiMalloc;
use numfmt::{Formatter, Precision, Scales};
use once_cell::sync::Lazy;
use crate::io::svg_util::{SvgDrawOptions, SvgLayoutTheme};

pub mod io;
pub mod sample;
mod overlap;
pub mod opt;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub static EPOCH: Lazy<Instant> = Lazy::new(Instant::now);

pub const OUTPUT_DIR: &str = "../output";

pub const SVG_OUTPUT_DIR: &str = "../output/svg";

pub const DRAW_OPTIONS: SvgDrawOptions = SvgDrawOptions{
    theme: SvgLayoutTheme::GRAY_THEME,
    quadtree: false,
    haz_prox_grid: false,
    surrogate: false,
    overlap_lines: true,
};

const FMT: Lazy<Formatter> = Lazy::new(
    || Formatter::new().scales(Scales::short()).precision(Precision::Significance(3))
);
