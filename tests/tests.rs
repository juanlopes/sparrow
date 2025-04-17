#[cfg(test)]
mod integration_tests {
    use jagua_rs::io::parser::Parser;
    use rand::prelude::SmallRng;
    use rand::SeedableRng;
    use sparrow::config::{CDE_CONFIG, LBF_SAMPLE_CONFIG, OUTPUT_DIR, SEP_CFG_EXPLORE, SIMPL_TOLERANCE, MIN_ITEM_SEPARATION};
    use sparrow::optimizer::lbf::LBFBuilder;
    use sparrow::optimizer::separator::Separator;
    use sparrow::optimizer::{compression_phase, exploration_phase, Terminator};
    use sparrow::util::io;
    use std::path::Path;
    use std::time::Duration;
    use test_case::test_case;
    use sparrow::util::io::to_sp_instance;

    const EXPLORE_TIMEOUT: Duration = Duration::from_secs(10);
    const COMPRESS_TIMEOUT: Duration = Duration::from_secs(10);
    const INSTANCE_BASE_PATH: &str = "data/input";
    const RNG_SEED: Option<usize> = Some(0); // fix seed for reproducibility
    
    #[test_case("swim.json"; "swim")]
    #[test_case("shirts.json"; "shirts")]
    #[test_case("trousers.json"; "trousers")]
    fn simulate_optimization(path: &str) {
        let input_file_path = format!("{INSTANCE_BASE_PATH}/{path}");
        let json_instance = io::read_json_instance(Path::new(&input_file_path));

        let parser = Parser::new(CDE_CONFIG, SIMPL_TOLERANCE, MIN_ITEM_SEPARATION);
        let any_instance = parser.parse(&json_instance);
        let instance = to_sp_instance(any_instance.as_ref()).expect("Expected SPInstance");

        println!("[TEST] loaded instance: {}", json_instance.name);

        let output_folder_path = format!("{OUTPUT_DIR}/tests_{}", json_instance.name);

        let rng = match RNG_SEED {
            Some(seed) => {
                println!("[TEST] using provided seed: {}", seed);
                SmallRng::seed_from_u64(seed as u64)
            }
            None => {
                let seed = rand::random();
                println!("[TEST] no seed provided, using: {}", seed);
                SmallRng::seed_from_u64(seed)
            }
        };

        let mut terminator = Terminator::new_without_ctrlc();
        terminator.set_timeout_from_now(EXPLORE_TIMEOUT);

        let builder = LBFBuilder::new(instance.clone(), CDE_CONFIG, rng, LBF_SAMPLE_CONFIG).construct();
        let mut separator = Separator::new(builder.instance, builder.prob, builder.rng, output_folder_path, 0, SEP_CFG_EXPLORE);

        let sols = exploration_phase(&instance, &mut separator, &terminator);
        let final_explore_sol = sols.last().expect("no solutions found during exploration");

        terminator.set_timeout_from_now(COMPRESS_TIMEOUT);
        compression_phase(&instance, &mut separator, final_explore_sol, &terminator);
    }
}