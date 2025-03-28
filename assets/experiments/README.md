## Experiments
*[UNDER CONSTRUCTION]*

#### Solution files of the experiments

The best solutions ever produced by `sparrow` for each of the instances are stored in [records](../records) folder.

The final solutions and logs for all experiments used in the performance comparison in the paper stored in the [data](data) folder.

#### Reproduction of comparative experiments
The experiments were all executed via GitHub Actions, on a self-hosted runner equipped with an AMD Ryzen 9 7950X CPU.
This system was running Ubuntu 20.04 LTS under WSL2 on Windows 11.
The exact commands to run a benchmark are defined in [single_bench.yml](../../.github/workflows/single_bench.yml).


The logs contain some important information:

```
[BENCH] git commit hash: 4d70ca7f468957a046a74bbb614b896f0ad463e3
[BENCH] system time: 2025-03-28T19:13:40.628237341Z
[BENCH] no seed provided, using: 12552852848582794543
[BENCH] starting bench for swim (13x8 runs across 16 cores, 1200s timelimit)
...
```

To exactly reproduce a benchmark ensure:
- Your Rust toolchain matches the one that was the most recent at the time of the experiment (system time in the logs).
- You are on the same commit as the one in the logs (git commit hash).
- The `RNG_SEED` constant in [src/config.rs](../../src/config.rs) is set to the same value as in the logs.