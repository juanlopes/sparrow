#!/bin/bash

# Remove any existing SVG file
rm output/.live_solution.svg

# Open the visualizer
open live_solution_visualizer.html

# Run the program with the 1th argument as input file and 2nd argument as time limit
cargo run --profile release-ultimate --bin main --features=live_solutions -- $1 $2