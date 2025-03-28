# sparrow 🪶
### State-of-the-art nesting 🪺 heuristic for 2D irregular strip packing
This optimization algorithm can be used to solve 2D irregular strip packing problems, also commonly referred to as nesting problems.
It builds on [`jagua-rs`](https://github.com/JeroenGar/jagua-rs), a collision detection engine for 2D irregular cutting & packing problems.
<p align="left">
    <img src="assets/sparrow_logo.png" alt="logo" height=100>
</p>

## Some example solutions
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
- [Rust](https://www.rust-lang.org/tools/install) ≥ 1.85
- [`jagua-rs`](https://github.com/JeroenGar/jagua-rs) submodule initialized and up to date:
```bash
git submodule update --init
```

## Usage

General usage:
```bash
cargo run --release  -- \
    [path to input JSON] \
    [timelimit in seconds]
```

Concrete example:
```bash
cargo run --release -- \
    libs/jagua-rs/assets/swim.json \
    120
```

This repo also contains a simple visualizer to monitor the optimization process live, open [live_viewer.html](assets/live/live_viewer.html) in a web browser,
and compile `sparrow` with the `live_svg` feature enabled:

```bash
rm assets/live/.live_solution.svg
open assets/live/live_viewer.html
cargo run --release --features=live_svg -- \
    libs/jagua-rs/assets/swim.json \
    120
```
![Demo of the live solution viewer](assets/demo.gif)

You can advance the algorithm to the next phase manually by pressing 'Ctrl + C' in the terminal.

## Input

This repository uses the same JSON format as [`jagua-rs`](https://github.com/JeroenGar/jagua-rs) to represent instances.
These are also available in Oscar Oliveira's [OR-Datasets repository](https://github.com/Oscar-Oliveira/OR-Datasets/tree/master/Cutting-and-Packing/2D-Irregular).

See [`jagua-rs` README](https://github.com/JeroenGar/jagua-rs?tab=readme-ov-file#input) for details on the input format.

## Output

Solutions are exported as SVG files in `output` folder. The final SVG solutions is saved as `output/final_{name}.svg`.

The SVG files are both a visualization and formal output of a solution as all original shapes and their exact transformations applied to them are defined within the SVG:
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
The [SVG spec](https://stackoverflow.com/questions/18582935/the-applying-order-of-svg-transforms) defines that the transformations are applied from right to left.
So here the item is first rotated and then translated.

By default, a range of intermediate (and infeasible) solutions will be exported in `output/sols_{name}`.
To disable this and export only a single final solution, compile with the `only_final_svg` feature:
```bash
cargo run --release --features=only_final_svg -- \
    libs/jagua-rs/assets/swim.json \
    120
```
## Targeting maximum performance

To build a binary with maximum performance, set the `target-cpu=native` compiler flag, switch to the nightly toolchain (required for [SIMD](https://doc.rust-lang.org/std/simd/index.html) support) and enable the `simd` feature:

```bash
  export RUSTFLAGS='-C target-cpu=native'
  export RUSTUP_TOOLCHAIN=nightly
  cargo run --release --features=simd,only_final_svg -- \
      libs/jagua-rs/assets/swim.json \
      120
```

## Testing
A suite of `debug_assert!()` checks are included throughout the codebase to verify the correctness of the heuristic.
These assertions are omitted in release builds to maximize performance, but are active in test builds.
Some basic integration tests are included that run the heuristic on a few example instances while all assertions are active:
```bash
  cargo test
```

Alternatively you can enable all `debug_assert!()` checks in release builds by running the tests with the `debug-release` profile:
```bash
cargo run --profile debug-release -- \
    libs/jagua-rs/assets/swim.json \
    120
```

## Development

This repo is meant to remain a faithful representation of the algorithm described in [TBA].
Therefore, only pull requests containing bug fixes and performance improvements which do not change the algorithm too significantly will be accepted.

Feel free to fork the repository if you want to experiment with different heuristics or want to expand the functionality.

## License

This project is licensed under Mozilla Public License 2.0 - see the [LICENSE](LICENSE) file for details.

## Acknowledgements

This project began development at the research group CODeS of [NUMA - KU Leuven](https://numa.cs.kuleuven.be/) and was funded by [Research Foundation - Flanders (FWO)](https://www.fwo.be/en/) (grant number: 1S71222N).
<p>
<img src="https://upload.wikimedia.org/wikipedia/commons/4/49/KU_Leuven_logo.svg" height="50px" alt="KU Leuven logo">
&nbsp;
<img src="https://upload.wikimedia.org/wikipedia/commons/9/97/Fonds_Wetenschappelijk_Onderzoek_logo_2024.svg" height="50px" alt="FWO logo">
</p>
