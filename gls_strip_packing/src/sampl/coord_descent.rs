use crate::sampl::evaluator::{SampleEval, SampleEvaluator};
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::primitives::point::Point;
use rand::prelude::SmallRng;
use rand::Rng;

pub const K_SUCC: fsize = 1.1;
pub const K_FAIL: fsize = 0.5;

pub const STEP_INIT_RATIO: fsize = 0.05; // 5% of items min dimension
pub const STEP_MIN_RATIO: fsize = 0.001; // 0.1% of items min dimension

pub fn coordinate_descent(
    start: (DTransformation, SampleEval),
    evaluator: &mut SampleEvaluator,
    min_dim: fsize,
    rng: &mut impl Rng,
) -> (DTransformation, SampleEval, usize) {

    let mut counter = 0;
    let step_limit = min_dim * STEP_MIN_RATIO;
    let mut current_step = min_dim * STEP_INIT_RATIO;

    let mut cd_state = CDState {
        state: start.0,
        eval: start.1,
        dir: None,
    };

    while current_step > step_limit {
        counter += 1;
        let min_child = match cd_state.dir {
            Some(_) => {
                cd_state.gen_directional_children(current_step, evaluator)
                    .into_iter()
                    .min_by_key(|c| c.eval)
                    .unwrap()
            }
            None => {
                cd_state.gen_all_children(current_step, evaluator)
                    .into_iter()
                    .min_by_key(|c| c.eval)
                    .unwrap()
            }
        };

        if min_child.eval < cd_state.eval {
            cd_state = min_child;
            current_step *= K_SUCC;
        } else {
            current_step *= K_FAIL;
        }
    }

    (cd_state.state, cd_state.eval, counter)
}

struct CDState {
    pub state: DTransformation,
    pub eval: SampleEval,
    pub dir: Option<CDDirection>,
}

impl CDState {

    pub fn gen_all_children(&self, step_size: fsize, evaluator: &mut SampleEvaluator) -> [CDState; 8]{
        CDDirection::all_directions()
            .map(|d| {
                let step = d.step(self.state.translation().into(), step_size);
                let child_state = DTransformation::new(self.state.rotation(), (step.0, step.1));
                CDState {
                    state: child_state,
                    eval: evaluator.eval(child_state),
                    dir: Some(d),
                }
            })
    }

    pub fn gen_directional_children(&self, step_size: fsize, evaluator: &mut SampleEvaluator) -> [CDState; 3]{
        self.dir.unwrap().neighboring()
            .map(|d| {
                let step = d.step(self.state.translation().into(), step_size);
                let child_state = DTransformation::new(self.state.rotation(), (step.0, step.1));
                CDState {
                    state: child_state,
                    eval: evaluator.eval(child_state),
                    dir: Some(d),
                }
            })
    }
}


#[derive(Debug, Clone, Copy)]
enum CDDirection {
    N,
    E,
    S,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl CDDirection {
    pub fn all_directions() -> [CDDirection; 8] {
        [
            CDDirection::N,
            CDDirection::E,
            CDDirection::S,
            CDDirection::W,
            CDDirection::NE,
            CDDirection::NW,
            CDDirection::SE,
            CDDirection::SW,
        ]
    }

    pub fn step(&self, p: Point, step_size: fsize) -> Point {
        let diag_step = fsize::sqrt(0.5) * step_size;
        match self {
            CDDirection::N => Point(p.0, p.1 + step_size),
            CDDirection::E => Point(p.0 + step_size, p.1),
            CDDirection::S => Point(p.0, p.1 - step_size),
            CDDirection::W => Point(p.0 - step_size, p.1),
            CDDirection::NE => Point(p.0 + diag_step, p.1 + diag_step),
            CDDirection::NW => Point(p.0 - diag_step, p.1 + diag_step),
            CDDirection::SW => Point(p.0 - diag_step, p.1 - diag_step),
            CDDirection::SE => Point(p.0 + diag_step, p.1 - diag_step),
        }
    }

    pub fn neighboring(&self) -> [CDDirection; 3] {
        //return the 3 neighboring directions (including self)
        match self {
            CDDirection::N => [CDDirection::N, CDDirection::NE, CDDirection::NW],
            CDDirection::E => [CDDirection::E, CDDirection::NE, CDDirection::SE],
            CDDirection::S => [CDDirection::S, CDDirection::SE, CDDirection::SW],
            CDDirection::W => [CDDirection::W, CDDirection::NW, CDDirection::SW],
            CDDirection::NE => [CDDirection::NE, CDDirection::N, CDDirection::E],
            CDDirection::NW => [CDDirection::NW, CDDirection::N, CDDirection::W],
            CDDirection::SW => [CDDirection::SW, CDDirection::S, CDDirection::W],
            CDDirection::SE => [CDDirection::SE, CDDirection::S, CDDirection::E],
        }
    }
}
