use crate::config::{CD_STEP_FAIL, CD_STEP_SUCCESS};
use crate::eval::sample_eval::{SampleEval, SampleEvaluator};
use jagua_rs::geometry::DTransformation;
use jagua_rs::geometry::primitives::Point;
use log::trace;
use rand::Rng;
use std::cmp::Ordering;
use std::fmt::Debug;

pub fn refine_coord_desc(
    (init_dt, init_eval): (DTransformation, SampleEval),
    evaluator: &mut impl SampleEvaluator,
    step_init: f32,
    step_limit: f32,
    rng: &mut impl Rng,
) -> (DTransformation, SampleEval) {
    let n_evals_init = evaluator.n_evals();
    let init_pos = init_dt.translation().into();
    let rot = init_dt.rotation.into();

    // Initialize the coordinate descent algorithm
    let mut cd = CoordinateDescent {
        pos: init_pos,
        eval: init_eval,
        axis: CDAxis::random(rng),
        steps: (step_init, step_init),
        step_limit,
    };

    // As long as new candidates are available, evaluate them and update the state
    while let Some([p0, p1]) = cd.ask() {
        let p0_eval = evaluator.eval(DTransformation::new(rot, p0.into()), Some(cd.eval));
        let p1_eval = evaluator.eval(DTransformation::new(rot, p1.into()), Some(cd.eval));

        let best = [(p0, p0_eval), (p1, p1_eval)].into_iter()
            .min_by_key(|(_, e)| *e).unwrap();

        cd.tell(best, rng);
        trace!("CD: {:?}", cd);
        debug_assert!(evaluator.n_evals() - n_evals_init < 1000, "coordinate descent exceeded 1000 evals");
    }
    trace!("CD: {} evals, t: ({:.3}, {:.3}) -> ({:.3}, {:.3}), eval: {:?}",evaluator.n_evals() - n_evals_init, init_pos.0, init_pos.1, cd.pos.0, cd.pos.1, cd.eval);
    (DTransformation::new(rot, cd.pos.into()), cd.eval)
}

#[derive(Debug)]
struct CoordinateDescent {
    pub pos: Point,
    pub eval: SampleEval,
    pub axis: CDAxis,
    pub steps: (f32, f32),
    pub step_limit: f32,
}

impl CoordinateDescent {
    pub fn tell(&mut self, (pos, eval): (Point, SampleEval), rng: &mut impl Rng) {
        let eval_cmp = eval.cmp(&self.eval);
        let better = eval_cmp == Ordering::Less;
        let worse = eval_cmp == Ordering::Greater;

        if !worse {
            (self.pos, self.eval) = (pos, eval);
        }

        // Multiply step size of active axis
        let m = if better { CD_STEP_SUCCESS } else { CD_STEP_FAIL };

        match self.axis {
            CDAxis::Horizontal => self.steps.0 *= m,
            CDAxis::Vertical => self.steps.1 *= m,
            CDAxis::ForwardDiag | CDAxis::BackwardDiag => {
                //since both axis are involved, adjust both steps but less severely
                self.steps.0 *= m.sqrt();
                self.steps.1 *= m.sqrt();
            }
        }

        if !better {
            self.axis = CDAxis::random(rng);
        }
    }

    pub fn ask(&self) -> Option<[Point; 2]> {
        let (sx, sy) = self.steps;

        if sx < self.step_limit && sy < self.step_limit {
            // Stop generating candidates if both steps have reached the limit
            None
        } else {
            // Generate two candidates on either side of the current position
            let p = self.pos;
            let c = match self.axis {
                CDAxis::Horizontal => [Point(p.0 + sx, p.1), Point(p.0 - sx, p.1)],
                CDAxis::Vertical => [Point(p.0, p.1 + sy), Point(p.0, p.1 - sy)],
                CDAxis::ForwardDiag => [Point(p.0 + sx, p.1 + sy), Point(p.0 - sx, p.1 - sy)],
                CDAxis::BackwardDiag => [Point(p.0 - sx, p.1 + sy), Point(p.0 + sx, p.1 - sy)],
            };
            Some(c)
        }
    }
}

#[derive(Clone, Debug, Copy)]
enum CDAxis {
    /// Left and right
    Horizontal,
    /// Up and down
    Vertical,
    /// Up-right and down-left
    ForwardDiag,
    /// Up-left and down-right
    BackwardDiag,
}

impl CDAxis {
    fn random(rng: &mut impl Rng) -> Self {
        match rng.random_range(0..4) {
            0 => CDAxis::Horizontal,
            1 => CDAxis::Vertical,
            2 => CDAxis::ForwardDiag,
            3 => CDAxis::BackwardDiag,
            _ => unreachable!(),
        }
    }
}