# sparrow 🪶🪺
*builds intricate nestings in any environment*
### A State-of-the-Art Heuristic for the 2D Irregular Strip Packing Problem

This optimization algorithm can be used to solve the 2D irregular strip packing problems, also commonly referred to as nesting problems.

It is built on top of [`jagua-rs`](https://github.com/JeroenGar/jagua-rs): *a collision detection engine for 2D irregular cutting & packing problems*.

## Example solutions
<p align="center">
    <img src="assets/records/final_best_trousers.svg" height=200/>
    <img src="assets/records/final_best_mao.svg" height=200/>
</p>
<p align="center">
    <img src="assets/records/final_best_swim.svg" height=250/>
    <img src="assets/records/final_best_marques.svg" height=250/>
    <img src="assets/records/final_best_dagli.svg" height=250/>
</p>
<p align="center">
    <img src="assets/records/final_best_albano.svg" height=220/>
    <img src="assets/records/final_best_shirts.svg" height=220/>
</p>

## Requirements
- Rust ≥ 1.85

This repository includes `jagua-rs` as a submodule.
Make sure it is initialized and up to date:
```bash
git submodule update --init
```

## Usage

General usage:
```bash
cargo run --release  -- \
    [path to input JSON] \
    [timelimit exploration phase in seconds]
```

Concrete example:
```bash
cargo run --release -- \
    libs/jagua-rs/assets/swim.json \
    120
```

If you want to view the optimization process live, open `assets/live_solution_viewer.html` in a web browser,
and compile with the `live_svg` feature enabled:

```bash
rm output/.live_solution.svg
open assets/live_solution_visualizer.html
cargo run --release --features=live_svg -- \
    [path to input JSON] \
    [timelimit exploration phase in seconds]
```
![Demo of the live solution viewer](assets/demo.gif)

For ultimate performance:
```bash
RUSTFLAGS='-C target-cpu=native'
cargo run --profile release -- \
    [path to input JSON] \
    [timelimit exploration phase in seconds]
```

## Input

🚧

## Output

Solutions will be exported as SVG files in `/output` folder.
These SVGs contain all the original shapes and transformations applied to them:
```html
    ...
    <g id="items">
        <defs>
            <g id="item_0">...</g>
        </defs>
        <use transform="translate(1289.9116 1828.7717), rotate(-90)" xlink:href="#item_0">...</use>
    </g>
    ...
```
