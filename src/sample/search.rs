use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::problem_generic::STRIP_LAYOUT_IDX;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::circle::Circle;
use log::{debug, info, trace};
use rand::Rng;
use crate::overlap::tracker::OverlapTracker;
use crate::sample::best_samples::BestSamples;
use crate::sample::coord_descent::coordinate_descent;
use crate::sample::eval::{SampleEval, SampleEvaluator};
use crate::sample::hpg_biased_sampler::HPGBiasedSampler;
use crate::sample::uniform_sampler::{UniformAARectSampler};

#[derive(Debug, Clone, Copy)]
pub struct SearchConfig {
    pub n_bin_samples: usize,
    pub n_focussed_samples: usize,
    pub n_coord_descents: usize,
}

pub fn search_placement(l: &Layout, item: &Item, ref_pk: Option<PItemKey>, mut evaluator: impl SampleEvaluator, search_config: SearchConfig, rng: &mut impl Rng) -> (DTransformation, SampleEval) {
    let item_min_dim = fsize::min(
        item.shape.as_ref().bbox().width(),
        item.shape.as_ref().bbox().height(),
    );

    let mut best_samples = BestSamples::new(search_config.n_coord_descents, item_min_dim * 0.05);

    let current = match ref_pk {
        Some(ref_pk) => {
            let current_transf = l.placed_items[ref_pk].d_transf;
            let current_eval = evaluator.eval(current_transf);

            best_samples.report(current_transf, current_eval);
            Some((current_transf, current_eval))
        }
        None => None,
    };

    //let bin_sampler = HPGBiasedSampler::new(item, l);
    let bin_sampler = UniformAARectSampler::new(l.bin.bbox(), item);

    for _ in 0..search_config.n_bin_samples {
        let transf = bin_sampler.sample(rng);
        let d_transf = transf.into();
        let eval = evaluator.eval(d_transf);
        best_samples.report(d_transf, eval);
    }

    if let Some((current_transf, _)) = current {
        let focussed_uni_sampler = {
            let center = current_transf.translation().into();
            let radius = item_min_dim * 0.5;
            let focussed_bbox = Circle::new(center, radius).bbox();
            UniformAARectSampler::new(focussed_bbox, item)
        };
        for _ in 0..search_config.n_focussed_samples {
            let d_transf = focussed_uni_sampler.sample(rng);
            let eval = evaluator.eval(d_transf);
            best_samples.report(d_transf, eval);
        }
    }


    for start in best_samples.samples.clone() {
        let n_evals_before = evaluator.n_evals();
        let descended = coordinate_descent(start.clone(), &mut evaluator, item_min_dim, rng);
        let n_evals_after = evaluator.n_evals();
        trace!("CD: {:?} -> {:?}, dt: {} -> {}, #s: {}",
            start.1,
            descended.1,
            start.0,
            descended.0,
            n_evals_after - n_evals_before,
        );
        best_samples.report(descended.0, descended.1);
    }

    debug!("[S] {} samples evaluated, best: {:?}, {}", evaluator.n_evals(), best_samples.best().1, best_samples.best().0);

    best_samples.best()
}

pub fn p_opts_are_unique(p_opt_1: &PlacingOption, p_opt_2: &PlacingOption, threshold: fsize) -> bool {
    let PlacingOption{item_id: id_1, d_transf: dt_1, layout_idx: _} = p_opt_1;
    let PlacingOption{item_id: id_2, d_transf: dt_2, layout_idx: _} = p_opt_2;

    if id_1 != id_2 {
        true
    }
    else {
        d_transfs_are_unique(*dt_1, *dt_2, threshold)
    }
}

pub fn d_transfs_are_unique(dt_1: DTransformation, dt_2: DTransformation, threshold: fsize) -> bool {
    let DTransformation{rotation: r_1, translation: (x_1, y_1)} = dt_1;
    let DTransformation{rotation: r_2, translation: (x_2, y_2)} = dt_2;

    if r_1 != r_2 {
        true
    }
    else {
        //check if the x and y translations are different enough
        let x_diff = (x_1 - x_2).abs();
        let y_diff = (y_1 - y_2).abs();

        x_diff > threshold || y_diff > threshold
    }
}
