#![allow(const_item_mutation)]

use mimalloc::MiMalloc;
use numfmt::{Formatter, Precision, Scales};
use once_cell::sync::Lazy;
use std::time::Instant;
pub mod optimizer;
pub mod overlap;
pub mod sample;
pub mod util;
pub mod config;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub static EPOCH: Lazy<Instant> = Lazy::new(Instant::now);

const FMT: Lazy<Formatter> = Lazy::new(|| {
    Formatter::new()
        .scales(Scales::short())
        .precision(Precision::Significance(3))
});


#[cfg(feature = "live_svg")]
pub const EXPORT_LIVE_SVG: bool = true;

#[cfg(not(feature = "live_svg"))]
pub const EXPORT_LIVE_SVG: bool = false;