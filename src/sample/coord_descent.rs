use crate::config::{CD_STEP_FAIL, CD_STEP_SUCCESS};
use crate::eval::sample_eval::{SampleEval, SampleEvaluator};
use jagua_rs::geometry::DTransformation;
use jagua_rs::geometry::primitives::Point;
use log::trace;
use rand::Rng;
use std::cmp::Ordering;
use std::fmt::Debug;

/// Refines an initial 'sample' (transformation and evaluation) into a local minimum using a coordinate descent inspired algorithm.
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

    // Initialize the coordinate descent.
    let mut cd = CoordinateDescent {
        pos: init_pos,
        eval: init_eval,
        axis: CDAxis::random(rng),
        steps: (step_init, step_init),
        step_limit,
    };

    // From the CD state, ask for two candidate positions to evaluate. If none provided, stop.
    while let Some([p0, p1]) = cd.ask() {
        // Evaluate the two candidates using the evaluator.
        let p0_eval = evaluator.eval(DTransformation::new(rot, p0.into()), Some(cd.eval));
        let p1_eval = evaluator.eval(DTransformation::new(rot, p1.into()), Some(cd.eval));
        
        let best = [(p0, p0_eval), (p1, p1_eval)].into_iter()
            .min_by_key(|(_, e)| *e).unwrap();

        // Report the best candidate to the coordinate descent state.
        cd.tell(best, rng);
        trace!("CD: {:?}", cd);
        debug_assert!(evaluator.n_evals() - n_evals_init < 1000, "coordinate descent exceeded 1000 evals");
    }
    trace!("CD: {} evals, t: ({:.3}, {:.3}) -> ({:.3}, {:.3}), eval: {:?}",evaluator.n_evals() - n_evals_init, init_pos.0, init_pos.1, cd.pos.0, cd.pos.1, cd.eval);
    // Return the best transformation found by the coordinate descent.
    (DTransformation::new(rot, cd.pos.into()), cd.eval)
}

#[derive(Debug)]
struct CoordinateDescent {
    /// The current position in the coordinate descent
    pub pos: Point,
    /// The current evaluation of the position
    pub eval: SampleEval,
    /// The current axis on which new candidates are generated
    pub axis: CDAxis,
    /// The current step size for x and y axes
    pub steps: (f32, f32),
    /// The limit for the step size, below which no more candidates are generated
    pub step_limit: f32,
}

impl CoordinateDescent {

    /// Generates candidates to be evaluated. 
    pub fn ask(&self) -> Option<[Point; 2]> {
        let (sx, sy) = self.steps;

        if sx < self.step_limit && sy < self.step_limit {
            // Stop generating candidates if both steps have reached the limit
            None
        } else {
            // Generate two candidates on either side of the current position, according to the active axis.
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
    
    /// Updates the coordinate descent state with the new position and evaluation.
    pub fn tell(&mut self, (pos, eval): (Point, SampleEval), rng: &mut impl Rng) {
        // Check if the reported evaluation is better or worse than the current one.
        let eval_cmp = eval.cmp(&self.eval);
        let better = eval_cmp == Ordering::Less;
        let worse = eval_cmp == Ordering::Greater;

        if !worse {
            // Update the current position if not worse
            (self.pos, self.eval) = (pos, eval);
        }

        // Determine the step size multiplier depending on whether the new evaluation is better or worse.
        let m = if better { CD_STEP_SUCCESS } else { CD_STEP_FAIL };

        // Apply the step size multiplier to the relevant steps for the current axis
        match self.axis {
            CDAxis::Horizontal => self.steps.0 *= m,
            CDAxis::Vertical => self.steps.1 *= m,
            CDAxis::ForwardDiag | CDAxis::BackwardDiag => {
                //Since both axis are involved, adjust both steps but less severely
                self.steps.0 *= m.sqrt();
                self.steps.1 *= m.sqrt();
            }
        }

        // Every time a state is not improved, the axis gets changed to a new random one.
        if !better {
            self.axis = CDAxis::random(rng);
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