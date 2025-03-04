# Nesting algo

State of the art nesting algorithm for 2D irregular shapes.
Built on top of the [`jagua-rs`](https://github.com/JeroenGar/jagua-rs) collision detection library.

## Requirements
- Rust 1.85 or later
- `jagua-rs` submodule initialized:
```bash
git submodule update --init --recursive
```

## Usage

Basic format:
```bash
cargo run --release  -- \
    <path to input json> \
    <time_limit> \
```

Concrete example:
```bash
cargo run --release -- \
    libs/jagua-rs/assets/swim.json \
    120
```

If you want to view the optimization process live, enable the `live_svg` feature and open `live_solution_viewer.html` in a web browser:

```bash
open live_solution_visualizer.html
cargo run --release --features=live_svg -- \
    libs/jagua-rs/assets/swim.json \
    120
```
![](assets/demo.gif)

For ultimate performance, compile with `release-ulimate` profile:
```bash
cargo run --profile release-ultimate -- \
    libs/jagua-rs/assets/swim.json \
    120
```
