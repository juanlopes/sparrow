use crate::eval::sample_eval::SampleEval;
use crate::sample::dtransfs_are_similar;
use itertools::Itertools;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use std::fmt::Debug;

//datastructure that stores the N best samples, automatically keeps them sorted and evicts the worst.
//it also makes sure that no two samples in its list are too similar
#[derive(Debug, Clone)]
pub struct BestSamples {
    pub size: usize,
    pub samples: Vec<(DTransformation, SampleEval)>,
    pub unique_thresh: fsize,
}

impl BestSamples {
    pub fn new(size: usize, unique_thresh: fsize) -> Self {
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

    pub fn upperbound(&self) -> SampleEval {
        self.samples.last().unwrap().1
    }
}
