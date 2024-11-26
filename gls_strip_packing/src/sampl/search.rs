use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::placed_item::PItemKey;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::problem_generic::STRIP_LAYOUT_IDX;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::Shape;
use log::debug;
use rand::Rng;
use crate::overlap::overlap_tracker::OverlapTracker;
use crate::sampl::best_samples::BestSamples;
use crate::sampl::coord_descent::coordinate_descent;
use crate::sampl::evaluator::{SampleEval, SampleEvaluator};
use crate::sampl::uniform_sampler::{UniformAARectSampler};

pub fn search_placement(l: &Layout, item: &Item, ref_pk: Option<PItemKey>, ot: &OverlapTracker, n_uniform: usize, n_cd: usize, rng: &mut impl Rng) -> (DTransformation, SampleEval) {
    let item_min_dim = fsize::min(
        item.shape.as_ref().bbox().width(),
        item.shape.as_ref().bbox().height(),
    );

    let mut evaluator = SampleEvaluator::new(l, item, ref_pk, ot);

    let mut best_samples = BestSamples::new(n_cd);

    if let Some(ref_pk) = ref_pk {
        let current_transf = l.placed_items[ref_pk].d_transf;
        let current_eval = evaluator.eval(current_transf);

        best_samples.report(current_transf, current_eval);
    }

    let uni_sampler = UniformAARectSampler::new(l.bin.bbox(), item);

    for _ in 0..n_uniform {
        let d_transf = uni_sampler.sample(rng);
        let eval = evaluator.eval(d_transf);
        best_samples.report(d_transf, eval);
    }

    for start in best_samples.samples.clone() {
        let descended = coordinate_descent(start.clone(), &mut evaluator, item_min_dim, rng);
        debug!("CD: {:?} -> {:?}, transl: {:.3} -> {:.3}, #{}",
            start.1,
            descended.1,
            start.0.translation(),
            descended.0.translation(),
            descended.2,
        );
        best_samples.report(descended.0, descended.1);
    }

    best_samples.take_best().unwrap()
}