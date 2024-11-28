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
use log::debug;
use rand::Rng;
use crate::overlap::overlap_tracker::OverlapTracker;
use crate::sampl::best_samples::BestSamples;
use crate::sampl::coord_descent::coordinate_descent;
use crate::sampl::evaluator::{SampleEval, SampleEvaluator};
use crate::sampl::uniform_sampler::{UniformAARectSampler};

pub struct SearchConfig {
    pub n_bin_samples: usize,
    pub n_focussed_samples: usize,
    pub n_coord_descents: usize,
}

pub fn search_placement(l: &Layout, item: &Item, ref_pk: Option<PItemKey>, ot: &OverlapTracker, search_config: SearchConfig, rng: &mut impl Rng) -> (DTransformation, SampleEval) {
    let item_min_dim = fsize::min(
        item.shape.as_ref().bbox().width(),
        item.shape.as_ref().bbox().height(),
    );

    let mut evaluator = SampleEvaluator::new(l, item, ref_pk, ot);

    let mut best_samples = BestSamples::new(search_config.n_coord_descents);

    let current = match ref_pk {
        Some(ref_pk) => {
            let current_transf = l.placed_items[ref_pk].d_transf;
            let current_eval = evaluator.eval(current_transf);

            best_samples.report(current_transf, current_eval);
            Some(current_transf)
        }
        None => None,
    };

    let bin_uni_sampler = UniformAARectSampler::new(l.bin.bbox(), item);
    for _ in 0..search_config.n_bin_samples {
        let d_transf = bin_uni_sampler.sample(rng);
        let eval = evaluator.eval(d_transf);
        best_samples.report(d_transf, eval);
    }

    if let Some(current) = current {
        let focussed_uni_sampler = {
            let center = current.translation().into();
            let radius = item_min_dim * 0.1;
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
        let descended = coordinate_descent(start.clone(), &mut evaluator, item_min_dim, rng);
        debug!("CD: {:?} -> {:?}, transl: ({:.3},{:.3}) -> ({:.3},{:.3}) #{}",
            start.1,
            descended.1,
            start.0.translation().0,
            start.0.translation().1,
            descended.0.translation().0,
            descended.0.translation().1,
            descended.2,
        );
        best_samples.report(descended.0, descended.1);
    }

    best_samples.take_best().unwrap()
}