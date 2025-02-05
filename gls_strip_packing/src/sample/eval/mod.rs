pub mod constructive_evaluator;
pub mod overlapping_evaluator;
pub mod hpg_eval;

use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::util::fpa::FPA;
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum SampleEval {
    Colliding { w_overlap: fsize, hpg_value: fsize },
    Valid(fsize),
    Invalid,
}

impl SampleEval {
    fn variant_index(&self) -> u8 {
        match self {
            SampleEval::Valid(_) => 0,
            SampleEval::Colliding { .. } => 1,
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
                (SampleEval::Colliding{w_overlap: wo1, hpg_value: hv1}, SampleEval::Colliding { w_overlap: wo2, hpg_value: hv2 }) => {
                    let d1 = wo1 / hv1;
                    let d2 = wo2 / hv2;
                    FPA(d1).partial_cmp(&FPA(d2))
                }
                (SampleEval::Valid(d1), SampleEval::Valid(d2)) => FPA(*d1).partial_cmp(&FPA(*d2)),
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
    fn eval(&mut self, dt: DTransformation) -> SampleEval;

    fn n_evals(&self) -> usize;
}
