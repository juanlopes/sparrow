#[cfg(test)]
mod integration_tests {
    use std::path::Path;
    use std::time::{Duration};
    use jagua_rs::entities::instances::instance::Instance;
    use jagua_rs::io::parser::Parser;
    use rand::prelude::SmallRng;
    use rand::SeedableRng;
    use sparrow::config::{CDE_CONFIG, LBF_SAMPLE_CONFIG, OUTPUT_DIR, SEP_CONFIG_EXPLORE, SIMPLIFICATION_CONFIG};
    use sparrow::optimizer::{compress, explore, Terminator};
    use sparrow::optimizer::lbf::LBFBuilder;
    use sparrow::optimizer::separator::Separator;
    use sparrow::util::io;
    use test_case::test_case;

    const EXPLORE_TIMEOUT: Duration = Duration::from_secs(20);
    const COMPRESS_TIMEOUT: Duration = Duration::from_secs(10);
    const INSTANCE_BASE_PATH: &str = "libs/jagua-rs/assets";
    const RNG_SEED: Option<usize> = Some(0); // fix seed for reproducibility
    
    #[test_case("swim.json"; "swim")]
    #[test_case("shirts.json"; "shirts")]
    #[test_case("trousers.json"; "trousers")]
    fn simulate_optimization(path: &str) {
        let input_file_path = format!("{INSTANCE_BASE_PATH}/{path}");
        let json_instance = io::read_json_instance(Path::new(&input_file_path));

        let parser = Parser::new(SIMPLIFICATION_CONFIG, CDE_CONFIG, true);
        let instance = match parser.parse(&json_instance){
            Instance::SP(spi) => spi,
            _ => panic!("expected strip packing instance"),
        };

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

        let mut terminator = Terminator::dummy();
        terminator.set_timeout(EXPLORE_TIMEOUT);

        let builder = LBFBuilder::new(instance.clone(), CDE_CONFIG, rng, LBF_SAMPLE_CONFIG).construct();
        let mut separator = Separator::new(builder.instance, builder.prob, builder.rng, output_folder_path, 0, SEP_CONFIG_EXPLORE);

        let sols = explore(&mut separator, &terminator);
        let final_explore_sol = sols.last().expect("no solutions found during exploration");

        terminator.set_timeout(COMPRESS_TIMEOUT);
        compress(&mut separator, final_explore_sol, &terminator);
    }
}