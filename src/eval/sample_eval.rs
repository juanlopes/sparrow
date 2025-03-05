use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::util::fpa::FPA;
use std::cmp::Ordering;

use SampleEval::{Clear, Collision, Invalid};

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum SampleEval {
    /// No collisions occur
    Clear { loss: f32 },
    Collision{ loss: f32 },
    Invalid,
}

impl PartialOrd for SampleEval {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Invalid, Invalid) => Some(Ordering::Equal),
            (Invalid, _) => Some(Ordering::Greater),
            (_, Invalid) => Some(Ordering::Less),
            (Collision{..}, Clear{..}) => Some(Ordering::Greater),
            (Clear{..}, Collision{..}) => Some(Ordering::Less),
            (Collision{loss: l1}, Collision{loss: l2}) |
            (Clear{loss: l1}, Clear { loss: l2 }) => {
                FPA(*l1).partial_cmp(&FPA(*l2))
            }
        }
    }
}

impl Ord for SampleEval {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Eq for SampleEval {}

/// Simple trait for structs that can evaluate samples
pub trait SampleEvaluator {
    fn eval(&mut self, dt: DTransformation, upper_bound: Option<SampleEval>) -> SampleEval;

    fn n_evals(&self) -> usize;
}