use crate::sample::eval::SampleEval;
use crate::sample::search;
use itertools::Itertools;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::transformation::Transformation;
use log::{debug, trace};
use std::fmt::Debug;

//datastructure that stores the N best samples, automatically keeps them sorted and evicts the worst
#[derive(Debug, Clone)]
pub struct BestSamples {
    pub samples: Vec<(DTransformation, SampleEval)>,
    pub unique_threshold: fsize,
}

impl BestSamples {
    pub fn new(capacity: usize, unique_threshold: fsize) -> Self {
        Self {
            samples: vec![(DTransformation::new(0.0, (0.0, 0.0)), SampleEval::Invalid); capacity],
            unique_threshold,
        }
    }

    pub fn report(&mut self, dt: DTransformation, eval: SampleEval) -> bool {
        if eval >= self.worst().1 {
            trace!("[BS] sample rejected, worse than upper bound: {:?} ({})", &eval, dt);
            false
        }
        else {
            let similar_sample_idx = self.samples.iter()
                .find_position(|(d, _)| !search::d_transfs_are_unique(*d, dt, self.unique_threshold));
            match similar_sample_idx {
                None => {
                    // no similar sample exists
                    trace!("[BS] sample accepted, unique: {:?} ({})", &eval, dt);
                    self.samples.pop();
                    self.samples.push((dt, eval));
                    self.samples.sort_by(|a, b| a.1.cmp(&b.1));
                    true
                }
                Some((idx, sim)) => {
                    //similar sample found
                    if eval < sim.1 {
                        trace!("[BS] sample accepted, better than similar: {:?} ({})", &eval, dt);
                        self.samples[idx] = (dt, eval);
                        self.samples.sort_by(|a, b| a.1.cmp(&b.1));
                        true
                    }
                    else {
                        trace!("sample rejected, worse than similar: {:?} ({})", &eval, dt);
                        false
                    }
                }
            }
        }
    }

    pub fn best(&self) -> (DTransformation, SampleEval) {
        self.samples.first().unwrap().clone()
    }

    pub fn worst(&self) -> (DTransformation, SampleEval) {
        self.samples.last().unwrap().clone()
    }
}
