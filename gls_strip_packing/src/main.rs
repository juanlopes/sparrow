use std::time::Instant;
use mimalloc::MiMalloc;
use once_cell::sync::Lazy;

mod io;
mod sampl;
mod overlap;
mod opt;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const OUTPUT_DIR: &str = "output";

pub static EPOCH: Lazy<Instant> = Lazy::new(Instant::now);

fn main() {
    println!("Hello, world!");
}
