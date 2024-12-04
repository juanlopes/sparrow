pub mod overlapping_evaluator;
pub mod ch_corner_evaluator;
pub mod ch_edge_evaluator;

use std::cmp::Ordering;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::util::fpa::FPA;

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum SampleEval{
    Colliding(usize, fsize),
    Valid(fsize)
}

impl PartialOrd for SampleEval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (SampleEval::Valid(s1), SampleEval::Valid(s2)) => FPA(*s1).partial_cmp(&FPA(*s2)),
            (SampleEval::Colliding(_, s1), SampleEval::Colliding(_, s2)) => FPA(*s1).partial_cmp(&FPA(*s2)),
            (SampleEval::Valid(_), _) => Some(Ordering::Less),
            (_, SampleEval::Valid(_)) => Some(Ordering::Greater),
        }
    }
}

impl Ord for SampleEval {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Eq for SampleEval {}

pub trait SampleEvaluator {
    fn eval(&mut self, dt: DTransformation) -> SampleEval;

    fn n_evals(&self) -> usize;
}
