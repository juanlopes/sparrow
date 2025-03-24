use std::f32::consts::PI;
use crate::eval::sample_eval::SampleEval;
use itertools::Itertools;
use jagua_rs::geometry::d_transformation::DTransformation;
use std::fmt::Debug;

/// Datastructure to store the N best samples, automatically keeps them sorted and evicts the worst.
/// It makes sure that no two included samples are too similar.
#[derive(Debug, Clone)]
pub struct BestSamples {
    pub size: usize,
    pub samples: Vec<(DTransformation, SampleEval)>,
    pub unique_thresh: f32,
}

impl BestSamples {
    pub fn new(size: usize, unique_thresh: f32) -> Self {
        Self {
            size,
            samples: vec![(DTransformation::empty(), SampleEval::Invalid); size],
            unique_thresh,
        }
    }

    pub fn report(&mut self, dt: DTransformation, eval: SampleEval) -> bool {
        let accepted = match eval < self.samples[self.size - 1].1 {
            false => false, //worse than worst
            true => {
                let similar_sample_idx = self.samples.iter()
                    .find_position(|(d, _)| dtransfs_are_similar(*d, dt, self.unique_thresh, self.unique_thresh));
                match similar_sample_idx {
                    None => { //no similar sample found, replace worst
                        self.samples[self.size - 1] = (dt, eval);
                        true
                    }
                    Some((idx, (_sim_dt, sim_eval))) => {
                        match eval < *sim_eval {
                            true => { //better than similar, replace
                                self.samples[idx] = (dt, eval);
                                true
                            }
                            false => false
                        }
                    }
                }
            }
        };
        if accepted { self.samples.sort_by_key(|(_, eval)| *eval); }
        accepted
    }

    pub fn best(&self) -> (DTransformation, SampleEval) {
        self.samples[0].clone()
    }

    pub fn worst(&self) -> SampleEval {
        self.samples.last().unwrap().1
    }
}

pub fn dtransfs_are_similar(
    dt1: DTransformation,
    dt2: DTransformation,
    x_threshold: f32,
    y_threshold: f32,
) -> bool {
    let x_diff = f32::abs(dt1.translation().0 - dt2.translation().0);
    let y_diff = f32::abs(dt1.translation().1 - dt2.translation().1);

    if x_diff < x_threshold && y_diff < y_threshold {
        let r1 = dt1.rotation() % 2.0 * PI;
        let r2 = dt2.rotation() % 2.0 * PI;
        let angle_diff = f32::abs(r1 - r2);
        angle_diff < (1.0f32).to_radians()
    } else {
        false
    }
}