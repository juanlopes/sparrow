# sparrow 🪶🪺
_Builds nests that make the most out of every inch of space._

### A State-of-the-Art Heuristic for the 2D Irregular Strip Packing Problem

This optimization algorithm can be used to solve 2D irregular strip packing problems, also commonly referred to as nesting problems.

It builts on [`jagua-rs`](https://github.com/JeroenGar/jagua-rs): *a collision detection engine for 2D irregular cutting & packing problems*.

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
    libs/jagua-rs/assets/swim.json \
    120
```
![Demo of the live solution viewer](assets/demo.gif)

To ensure maximum performance, compile with:
```bash
RUSTFLAGS='-C target-cpu=native'
```

## Input

This repository uses the same JSON format as [`jagua-rs`](https://github.com/JeroenGar/jagua-rs) to represent instances.
These are also available in Oscar Oliveira's [OR-Datasets repository](https://github.com/Oscar-Oliveira/OR-Datasets/tree/master/Cutting-and-Packing/2D-Irregular).

See [`jagua-rs` README](https://github.com/JeroenGar/jagua-rs?tab=readme-ov-file#input) for details on the input format.

## Output

Solutions are exported as SVG files in `/output` folder.
The SVG files are both a visualization and a formal output of a solution as they define all the original shapes and the exact transformations applied to them:
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
The final SVG solutions is saved as `output/final_{name}.svg`.

By default, a range of intermediate (and infeasible) solutions will be exported into `/output/sols_{name}`.

To disable this and export only a single final solution, enable the `only_final_svg` feature:
```bash
cargo run --release --features=only_final_svg -- \
    libs/jagua-rs/assets/swim.json \
    120
```

## Testing
The codebase contains a suite of `debug_assert!` checks to verify the correctness of the heuristic.
These assertions are omitted in release builds to maximize performance, but can be enabled by running the algorithm in the `debug-release` profile:
```bash
cargo run --profile debug-release -- \
    libs/jagua-rs/assets/swim.json \
    120
```

## Development

This repo is meant to remain a faithful representation of the algorithm described in [...].
Therefore, only pull requests containing performance improvements and bug fixes will be accepted.
Feel free to fork the repository if you want to experiment with different heuristics.

## License

This project is licensed under Mozilla Public License 2.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgements

This project began development at [KU Leuven](https://www.kuleuven.be/english/) and was funded by [Research Foundation - Flanders (FWO)](https://www.fwo.be/en/) (grant number: 1S71222N).

<img src="https://upload.wikimedia.org/wikipedia/commons/9/97/Fonds_Wetenschappelijk_Onderzoek_logo_2024.svg" height="50px" alt="FWO logo">
&nbsp;
<img src="https://upload.wikimedia.org/wikipedia/commons/4/49/KU_Leuven_logo.svg" height="50px" alt="KU Leuven logo">