use crate::util::io::svg_util::{SvgDrawOptions, SvgLayoutTheme};
use mimalloc::MiMalloc;
use numfmt::{Formatter, Precision, Scales};
use once_cell::sync::Lazy;
use std::time::Instant;
pub mod opt;
pub mod overlap;
pub mod sample;
pub mod util;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub static EPOCH: Lazy<Instant> = Lazy::new(Instant::now);

pub const OUTPUT_DIR: &str = "output";

pub const SVG_OUTPUT_DIR: &str = "output/svg";

pub const DRAW_OPTIONS: SvgDrawOptions = SvgDrawOptions {
    theme: SvgLayoutTheme::GRAY_THEME,
    quadtree: false,
    haz_prox_grid: false,
    surrogate: false,
    overlap_lines: true,
};

const FMT: Lazy<Formatter> = Lazy::new(|| {
    Formatter::new()
        .scales(Scales::short())
        .precision(Precision::Significance(3))
});
