use crate::sample::eval::{SampleEval, SampleEvaluator};
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::primitives::point::Point;
use log::trace;
use rand::Rng;
use std::cmp::Ordering;
use std::fmt::Debug;

pub const K_SUCC: fsize = 1.1;
pub const K_FAIL: fsize = 0.5;

pub const STEP_INIT_RATIO: fsize = 0.25; //25% of the item's min dimension

pub const STEP_LIMIT_RATIO: fsize = 0.001; //0.1% of the item's min dimension

pub fn coordinate_descent(
    (init_dt, init_eval): (DTransformation, SampleEval),
    evaluator: &mut impl SampleEvaluator,
    min_dim: fsize,
    rng: &mut impl Rng,
) -> (DTransformation, SampleEval) {
    let mut counter = 0;
    let init_pos = init_dt.translation().into();
    let rot = init_dt.rotation.into();

    let mut cd_state = CDState {
        pos: init_pos,
        eval: init_eval,
        axis: AXES[rng.random_range(0..4)],
        steps: (min_dim * STEP_INIT_RATIO, min_dim * STEP_INIT_RATIO),
        step_limit: min_dim * STEP_LIMIT_RATIO,
    };

    while let Some([c0, c1]) = cd_state.gen_candidates() {
        //evaluate the candidates
        let c0_eval = evaluator.eval(DTransformation::new(rot, c0.into()));
        let c1_eval = evaluator.eval(DTransformation::new(rot, c1.into()));

        let c0_cmp = c0_eval.cmp(&cd_state.eval);
        let c1_cmp = c1_eval.cmp(&cd_state.eval);

        trace!("CD: {:?}", cd_state);

        cd_state = match (c0_cmp, c1_cmp) {
            (Ordering::Less, Ordering::Less) => {
                //both are better, go to the best
                let move_to = [(c0, c0_eval), (c1, c1_eval)].iter()
                    .min_by_key(|(_, eval)| *eval).unwrap().clone();

                cd_state.evolve(Some(move_to), true)
            }
            (Ordering::Less, _) => cd_state.evolve(Some((c0, c0_eval.clone())), true),
            (_, Ordering::Less) => cd_state.evolve(Some((c1, c1_eval.clone())), true),
            (Ordering::Equal, Ordering::Equal) => {
                cd_state.evolve(Some((c0, c0_eval.clone())), false)
            }
            (Ordering::Equal, _) => cd_state.evolve(Some((c0, c0_eval.clone())), false),
            (_, Ordering::Equal) => cd_state.evolve(Some((c1, c1_eval.clone())), false),
            (_, _) => {
                //both are worse, switch axis and decrease step
                cd_state.evolve(None, false)
            }
        };
        counter += 2;
        assert!(
            counter < 100_000,
            "[CD] too many iterations, CD: {:?}, init: ({:.3}, {:.3})",
            cd_state,
            init_pos.0,
            init_pos.1
        );
    }
    trace!(
        "CD: {} evals, t: ({:.3}, {:.3}) -> ({:.3}, {:.3}), eval: {:?}",
        counter, init_pos.0, init_pos.1, cd_state.pos.0, cd_state.pos.1, cd_state.eval
    );
    let cd_d_transf = DTransformation::new(rot, cd_state.pos.into());
    (cd_d_transf, cd_state.eval)
}

#[derive(Debug)]
struct CDState<T: PartialOrd + Debug + Sized> {
    pub pos: Point,
    pub eval: T,
    pub axis: CDAxis,
    pub steps: (fsize, fsize),
    pub step_limit: fsize,
}

impl<T: PartialOrd + Debug> CDState<T> {
    pub fn evolve(mut self, new_pos: Option<(Point, T)>, improved: bool) -> Self {
        debug_assert!(new_pos.is_some() || !improved, "improved without new pos");
        //update the position
        (self.pos, self.eval) = new_pos.unwrap_or((self.pos, self.eval));

        self.adjust_steps(improved);

        if !improved {
            self.axis.cycle();
        }
        self
    }

    fn adjust_steps(&mut self, improved: bool) {
        let m = if improved { K_SUCC } else { K_FAIL };
        let (sx, sy) = self.steps;

        self.steps = match self.axis {
            CDAxis::Horizontal => (sx * m, sy),
            CDAxis::Vertical => (sx, sy * m),
            //since both axis are involved, adjust both steps but less severely
            CDAxis::DiagForward | CDAxis::DiagBackward => (sx * m.sqrt(), sy * m.sqrt()),
        };
    }

    pub fn gen_candidates(&self) -> Option<[Point; 2]> {
        let p = self.pos;
        let (sx, sy) = self.steps;

        if sx < self.step_limit && sy < self.step_limit {
            None
        } else {
            let c = match self.axis {
                CDAxis::Horizontal => [Point(p.0 + sx, p.1), Point(p.0 - sx, p.1)],
                CDAxis::Vertical => [Point(p.0, p.1 + sy), Point(p.0, p.1 - sy)],
                CDAxis::DiagForward => [Point(p.0 + sx, p.1 + sy), Point(p.0 - sx, p.1 - sy)],
                CDAxis::DiagBackward => [Point(p.0 - sx, p.1 + sy), Point(p.0 + sx, p.1 - sy)],
            };
            Some(c)
        }
    }
}

const AXES: [CDAxis; 4] = [
    CDAxis::Horizontal,
    CDAxis::Vertical,
    CDAxis::DiagForward,
    CDAxis::DiagBackward,
];

#[derive(Clone, Debug, Copy)]
enum CDAxis {
    //left and right
    Horizontal,
    //up and down
    Vertical,
    //up-right and down-left
    DiagForward,
    //up-left and down-right
    DiagBackward,
}

impl CDAxis {
    fn cycle(&mut self) {
        *self = match self {
            CDAxis::Horizontal => CDAxis::Vertical,
            CDAxis::Vertical => CDAxis::DiagForward,
            CDAxis::DiagForward => CDAxis::DiagBackward,
            CDAxis::DiagBackward => CDAxis::Horizontal,
        }
    }
}
