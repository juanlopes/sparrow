use clap::Parser;
#[derive(Parser)]
pub struct MainCli {
    /// Path to input file (mandatory)
    #[arg(short = 'i', long, help = "Path to the input JSON file")]
    pub input: String,

    /// Global time limit in seconds (mutually exclusive with -e and -c)
    #[arg(short = 't', long, conflicts_with_all = &["exploration", "compression"], help = "Set a global time limit (in seconds)")]
    pub global_time: Option<u64>,

    /// Exploration time limit in seconds (requires compression time)
    #[arg(short = 'e', long, requires = "compression", help = "Set the exploration phase time limit (in seconds)")]
    pub exploration: Option<u64>,

    /// Compression time limit in seconds (requires exploration time)
    #[arg(short = 'c', long, requires = "exploration", help = "Set the compression phase time limit (in seconds)")]
    pub compression: Option<u64>,
}
