pub mod constructive_evaluator;
pub mod overlapping_evaluator;

use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::util::fpa::FPA;
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum SampleEval {
    Valid(fsize),
    Colliding(fsize),
    Invalid,
}

impl SampleEval {
    fn variant_index(&self) -> u8 {
        match self {
            SampleEval::Valid(_) => 0,
            SampleEval::Colliding(_) => 1,
            SampleEval::Invalid => 2,
        }
    }
}

impl PartialOrd for SampleEval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match u8::cmp(&self.variant_index(), &other.variant_index()) {
            Ordering::Less => Some(Ordering::Less),
            Ordering::Greater => Some(Ordering::Greater),
            Ordering::Equal => match (self, other) {
                (SampleEval::Colliding(wo1), SampleEval::Colliding(wo2)) => {
                    FPA(*wo1).partial_cmp(&FPA(*wo2))
                }
                (SampleEval::Valid(v1), SampleEval::Valid(v2)) => FPA(*v1).partial_cmp(&FPA(*v2)),
                (SampleEval::Invalid, SampleEval::Invalid) => Some(Ordering::Equal),
                _ => unreachable!(),
            },
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
