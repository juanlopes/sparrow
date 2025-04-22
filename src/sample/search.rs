use jagua_rs::entities::general::{Item, Layout, PItemKey};
use jagua_rs::geometry::DTransformation;
use crate::config::{FIN_REF_CD_RATIOS, PRE_REF_CD_RATIOS, UNIQUE_SAMPLE_THRESHOLD};
use crate::eval::sample_eval::{SampleEval, SampleEvaluator};
use crate::sample::best_samples::BestSamples;
use crate::sample::coord_descent::refine_coord_desc;
use crate::sample::uniform_sampler::UniformBBoxSampler;
use jagua_rs::geometry::geo_traits::Shape;
use log::debug;
use rand::Rng;

#[derive(Debug, Clone, Copy)]
pub struct SampleConfig {
    pub n_bin_samples: usize,
    pub n_focussed_samples: usize,
    pub n_coord_descents: usize,
}

pub fn search_placement(l: &Layout, item: &Item, ref_pk: Option<PItemKey>, mut evaluator: impl SampleEvaluator, sample_config: SampleConfig, rng: &mut impl Rng) -> (Option<(DTransformation, SampleEval)>, usize) {
    let item_min_dim = f32::min(item.shape_cd.bbox().width(), item.shape_cd.bbox().height());

    let mut best_samples = BestSamples::new(sample_config.n_coord_descents, item_min_dim * UNIQUE_SAMPLE_THRESHOLD);

    let focussed_sampler = match ref_pk {
        Some(ref_pk) => {
            //report the current placement (and eval)
            let dt = l.placed_items[ref_pk].d_transf;
            let eval = evaluator.eval(dt, Some(best_samples.upper_bound()));

            debug!("[S] Starting from: {:?}", (dt, eval));
            best_samples.report(dt, eval);

            //create a sampler around the current placement
            let pi_bbox = l.placed_items[ref_pk].shape.bbox();
            UniformBBoxSampler::new(pi_bbox, item, l.bin.outer_cd.bbox())
        }
        None => None,
    };

    if let Some(focussed_sampler) = focussed_sampler {
        for _ in 0..sample_config.n_focussed_samples {
            let dt = focussed_sampler.sample(rng);
            let eval = evaluator.eval(dt, Some(best_samples.upper_bound()));
            best_samples.report(dt, eval);
        }
    }

    let bin_sampler = UniformBBoxSampler::new(l.bin.outer_cd.bbox(), item, l.bin.outer_cd.bbox());

    if let Some(bin_sampler) = bin_sampler {
        for _ in 0..sample_config.n_bin_samples {
            let dt = bin_sampler.sample(rng).into();
            let eval = evaluator.eval(dt, Some(best_samples.upper_bound()));
            best_samples.report(dt, eval);
        }
    }

    //Prerefine the best samples
    for start in best_samples.samples.clone() {
        let descended = refine_coord_desc(
            start.clone(),
            &mut evaluator,
            item_min_dim * PRE_REF_CD_RATIOS.0,
            item_min_dim * PRE_REF_CD_RATIOS.1,
            rng);
        best_samples.report(descended.0, descended.1);
    }


    //Do a final refine on the best one
    let final_sample = best_samples.best().map(|s|
        refine_coord_desc(s, &mut evaluator, item_min_dim * FIN_REF_CD_RATIOS.0, item_min_dim * FIN_REF_CD_RATIOS.1, rng)
    );

    debug!("[S] {} samples evaluated, final: {:?}",evaluator.n_evals(),final_sample);
    (final_sample, evaluator.n_evals())
}
