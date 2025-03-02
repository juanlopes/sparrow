use crate::sample::best_samples::BestSamples;
use crate::sample::coord_descent::coordinate_descent;
use crate::sample::eval::{SampleEval, SampleEvaluator};
use crate::sample::uniform_sampler::UniformBBoxSampler;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::Shape;
use log::debug;
use rand::Rng;

#[derive(Debug, Clone, Copy)]
pub struct SampleConfig {
    pub n_bin_samples: usize,
    pub n_focussed_samples: usize,
    pub n_coord_descents: usize,
}

pub fn search_placement(l: &Layout, item: &Item, ref_pk: Option<PItemKey>, mut evaluator: impl SampleEvaluator, sample_config: SampleConfig, rng: &mut impl Rng) -> (DTransformation, SampleEval, usize) {
    let item_min_dim = fsize::min(item.shape.bbox().width(), item.shape.bbox().height());

    let mut best_samples = BestSamples::new(sample_config.n_coord_descents, item_min_dim * 0.1);

    let focussed_sampler = match ref_pk {
        Some(ref_pk) => {
            //report the current placement (and eval)
            let dt = l.placed_items[ref_pk].d_transf;
            let eval = evaluator.eval(dt, Some(best_samples.worst().1));

            best_samples.report(dt, eval);

            //create a sampler around the current placement
            let pi_bbox = l.placed_items[ref_pk].shape.bbox();
            Some(UniformBBoxSampler::new(pi_bbox, item))
        }
        None => None,
    };

    let bin_sampler = UniformBBoxSampler::new(
        l.bin.bbox().resize_by(-item.shape.poi.radius, -item.shape.poi.radius),
        item,
    );

    for _ in 0..sample_config.n_bin_samples {
        let dt = bin_sampler.sample(rng).into();
        let eval = evaluator.eval(dt, Some(best_samples.worst().1));
        best_samples.report(dt, eval);
    }

    if let Some(focussed_sampler) = focussed_sampler {
        for _ in 0..sample_config.n_focussed_samples {
            let dt = focussed_sampler.sample(rng);
            let eval = evaluator.eval(dt, Some(best_samples.worst().1));
            best_samples.report(dt, eval);
        }
    }

    for start in best_samples.samples.clone() {
        let descended = coordinate_descent(start.clone(), &mut evaluator, item_min_dim, rng);
        best_samples.report(descended.0, descended.1);
    }

    debug!("[S] {} samples evaluated, best: {:?}, {}",evaluator.n_evals(),best_samples.best().1,best_samples.best().0);

    let best_sample = best_samples.best();
    (best_sample.0, best_sample.1, evaluator.n_evals())
}
