pub mod constructive_evaluator;
pub mod overlapping_evaluator;

use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::util::fpa::FPA;
use std::cmp::Ordering;

use SampleEval::{Colliding, Invalid, Valid};

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum SampleEval {
    Valid(fsize),
    Colliding(fsize),
    Invalid,
}

impl PartialOrd for SampleEval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other){
            (Invalid, Invalid) => Some(Ordering::Equal),
            (Invalid, _) => Some(Ordering::Greater),
            (_, Invalid) => Some(Ordering::Less),
            (Colliding(_), Valid(_)) => Some(Ordering::Greater),
            (Valid(_), Colliding(_)) => Some(Ordering::Less),
            (Colliding(wo1), Colliding(wo2)) => FPA(*wo1).partial_cmp(&FPA(*wo2)),
            (Valid(v1), Valid(v2)) => FPA(*v1).partial_cmp(&FPA(*v2)),
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
    fn eval(&mut self, dt: DTransformation, upper_bound: Option<SampleEval>) -> SampleEval;

    fn n_evals(&self) -> usize;
}
