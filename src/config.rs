use crate::sample::search::SearchConfig;
use crate::util::io::svg_util::{SvgDrawOptions, SvgLayoutTheme};
use jagua_rs::fsize;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use crate::optimizer::separator::SeparatorConfig;

pub const OUTPUT_DIR: &str = "output";

pub const LOG_LEVEL_RELEASE: log::LevelFilter = log::LevelFilter::Info;

pub const LOG_LEVEL_DEBUG: log::LevelFilter = log::LevelFilter::Debug;

pub const RNG_SEED: Option<usize> = Some(1);

pub const DRAW_OPTIONS: SvgDrawOptions = SvgDrawOptions {
    theme: SvgLayoutTheme::GRAY_THEME,
    quadtree: false,
    haz_prox_grid: false,
    surrogate: false,
    highlight_overlap: true,
};

pub const CDE_CONFIG: CDEConfig = CDEConfig {
    quadtree_depth: 3,
    hpg_n_cells: 0,
    item_surrogate_config: SPSurrogateConfig {
        pole_coverage_goal: 0.95,
        max_poles: 20,
        n_ff_poles: 4,
        n_ff_piers: 0,
    },
};

pub const CONSTR_SEARCH_CONFIG: SearchConfig = SearchConfig {
    n_bin_samples: 1000,
    n_focussed_samples: 0,
    n_coord_descents: 3,
};

pub const SEPARATOR_CONFIG_EXPLORE: SeparatorConfig = SeparatorConfig {
    iter_no_imprv_limit: 100,
    strike_limit: 5,
    jump_cooldown: 5,
    log_level: log::Level::Info,
    n_workers: 2,
    large_area_ch_area_cutoff_ratio: 0.5,
};

pub const SEARCH_N_BIN_SAMPLES: usize = 50;
pub const SEARCH_N_FOCUSSED_SAMPLES: usize = 25;
pub const SEARCH_N_COORD_DESCENTS: usize = 3;
pub const WEIGHT_MAX_INC_RATIO: fsize = 2.0;
pub const WEIGHT_MIN_INC_RATIO: fsize = 1.2;
pub const WEIGHT_OVERLAP_DECAY: fsize = 0.95;
pub const OVERLAP_PROXY_EPSILON_DIAM_RATIO: fsize = 0.01;
pub const EXPLORE_SOL_DISTR_STDDEV: fsize = 0.25;
pub const EXPLORE_R_SHRINK: fsize = 0.005;
pub const COMPRESS_R_SHRINKS: [fsize; 2] = [0.0005, 0.0001];
pub const COMPRESS_N_STRIKES: [usize; 2] = [5, 5];

pub const SEPARATOR_CONFIG_COMPRESS: SeparatorConfig = SeparatorConfig {
    iter_no_imprv_limit: 100,
    strike_limit: 5,
    jump_cooldown: 5,
    log_level: log::Level::Debug,
    n_workers: 2,
    large_area_ch_area_cutoff_ratio: 0.5,
};