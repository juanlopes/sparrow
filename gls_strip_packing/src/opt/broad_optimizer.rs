use std::cmp::Reverse;
use std::path::Path;
use std::time::{Duration, Instant};
use itertools::Itertools;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::instances::strip_packing::SPInstance;
use jagua_rs::entities::placing_option::PlacingOption;
use jagua_rs::entities::problems::problem_generic::ProblemGeneric;
use jagua_rs::entities::problems::strip_packing::SPProblem;
use jagua_rs::entities::solution::Solution;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::Shape;
use log::{warn, LevelFilter};
use ordered_float::OrderedFloat;
use rand::prelude::SmallRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::IntoParallelRefMutIterator;
use crate::opt::constr_builder::ConstructiveBuilder;
use rayon::iter::ParallelIterator;
use rayon::iter::Flatten;
use tap::Tap;
use crate::opt::constr_builder;
use crate::sampl::search;
use crate::{io, DRAW_OPTIONS, SVG_OUTPUT_DIR};
use crate::io::layout_to_svg::layout_to_svg;

const N_STEPS: usize = 20;
const STEP_TIMEOUT: Duration = Duration::from_secs(10);
pub struct BroadOptimizer {
    pub master_prob: SPProblem,
    pub builders: Vec<ConstructiveBuilder>,
    pub instance: SPInstance,
    pub rng: SmallRng,
    pub svg_counter: usize,
}

impl BroadOptimizer {
    pub fn new(n_threads: usize, mut prob: SPProblem, instance: SPInstance, mut rng: SmallRng) -> Self {
        let builders = (0..n_threads)
            .map(|_| ConstructiveBuilder::new(prob.clone(), instance.clone(), SmallRng::from_seed(rng.gen())))
            .collect();

        let master_prob = prob;

        Self {
            master_prob,
            builders,
            instance,
            rng,
            svg_counter: 0,
        }
    }

    pub fn optimize(&mut self){

        let mut selected_p_opts = vec![];
        let mut prev_rollouts = vec![];

        while selected_p_opts.len() < N_STEPS {
            let start = self.master_prob.create_solution(None);
            let mut rollouts = self.rollout(&start, STEP_TIMEOUT);
            rollouts.extend(prev_rollouts.into_iter());
            let n_rollouts = rollouts.len();
            let mut buckets = bucket_rollouts(rollouts.into_iter(), &self.instance);

            dbg!(n_rollouts, buckets.len());

            buckets.sort_by_key(|b| {
                let penalty = match b.less_than_5 {
                    true => 0.98,
                    false => 1.0,
                };
                Reverse(OrderedFloat(b.best_5_avg_eval * penalty))
            });
            for bucket in buckets.iter(){
                let n_rollouts = bucket.rollouts.len();
                dbg!(n_rollouts, bucket.best_eval, bucket.avg_eval, bucket.best_5_avg_eval);
                self.master_prob.place_item(bucket.p_opt.clone());
                self.write_svg(LevelFilter::Debug);
                self.master_prob.restore_to_solution(&start)
            }

            let best_bucket = &buckets[0];

            let converted_rollouts = best_bucket.rollouts.iter()
                .map(|r| {
                    let rollouts_except_first = r.rollout_p_opts.iter().skip(1).cloned().collect_vec();
                    Rollout::new(rollouts_except_first, &self.instance)
                });
            prev_rollouts = converted_rollouts.collect_vec();

            let selected_p_opt = best_bucket.p_opt.clone();
            selected_p_opts.push(selected_p_opt.clone());
            self.master_prob.place_item(selected_p_opt);
            self.write_svg(LevelFilter::Info);
        }
    }

    pub fn rollout(&mut self, init: &Solution, timeout: Duration) -> Vec<Rollout>{

        let start = Instant::now();
        let rollouts: Vec<_> = self.builders.par_iter_mut()
            .map(|b| {
                let mut rollouts = vec![];
                while start.elapsed() < timeout {
                    b.restore(&init);
                    let p_opts = b.fill();
                    let rollout = Rollout::new(p_opts, &self.instance);
                    rollouts.push(rollout);
                }
                rollouts
            })
            .flatten()
            .collect();

        rollouts
    }

    pub fn write_svg(&mut self, log_level: LevelFilter) {
        //skip if this log level is ignored by the logger
        if log_level > log::max_level() {
            return;
        }

        if self.svg_counter == 0 {
            //remove all .svg files from the output folder
            let _ = std::fs::remove_dir_all(SVG_OUTPUT_DIR);
            std::fs::create_dir_all(SVG_OUTPUT_DIR).unwrap();
        }

        let layout = &self.master_prob.layout;
        let filename = format!(
            "{}/{}_{:.2}.svg",
            SVG_OUTPUT_DIR,
            self.svg_counter,
            layout.bin.bbox().x_max
        );
        io::write_svg(
            &layout_to_svg(layout, &self.instance, DRAW_OPTIONS),
            Path::new(&filename),
        );
        self.svg_counter += 1;
        warn!("wrote layout to disk: file:///{}", filename);
    }
}

struct Rollout {
    rollout_p_opts: Vec<PlacingOption>,
    eval: fsize
}

impl Rollout {
    fn new(rollout_p_opts: Vec<PlacingOption>, instance: &SPInstance) -> Self {
        //give value to each included item
        let eval: fsize = rollout_p_opts.iter()
            .map(|p_opt| p_opt.item_id)
            .map(|item_id| &instance.item(item_id).shape)
            .map(|s| constr_builder::value_item(s))
            .sum();

        Self {
            eval,
            rollout_p_opts
        }
    }
}

pub struct RolloutBucket{
    p_opt: PlacingOption,
    rollouts: Vec<Rollout>,
    avg_eval: fsize,
    best_eval: fsize,
    best_5_avg_eval: fsize,
    less_than_5: bool
}

pub fn bucket_rollouts(rollouts: impl Iterator<Item=Rollout>, instance: &SPInstance) -> Vec<RolloutBucket>{
    let mut buckets: Vec<(PlacingOption, Vec<Rollout>)> = vec![];
    for rollout in rollouts {
        let first_p_opt = rollout.rollout_p_opts.first().unwrap();
        let item_diameter = instance.item(first_p_opt.item_id).shape.diameter();
        let matching_bucket_idx = buckets.iter().position(|(p_opt, _)|{
            !search::p_opts_are_unique(&p_opt, &first_p_opt, item_diameter * 0.05)
        });
        match matching_bucket_idx{
            Some(idx) => {
                let (_, bucket) = &mut buckets[idx];
                bucket.push(rollout);
            }
            None => {
                buckets.push((first_p_opt.clone(), vec![rollout]));
            }
        }
    }

    buckets.into_iter()
        .map(|(p_opt, rollouts)|{
            let n_rollouts = rollouts.len();
            let avg_eval = rollouts.iter().map(|r| r.eval).sum::<fsize>() / n_rollouts as fsize;
            let best_eval = rollouts.iter().map(|r| r.eval).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
            let best_5_avg_eval = rollouts.iter()
                .map(|r| r.eval)
                .sorted_by_key(|eval| Reverse(OrderedFloat(*eval)))
                .take(5)
                .fold((0, 0.0), |(n, sum), eval| (n + 1, sum + eval));

            let best_5_avg_eval = best_5_avg_eval.1 / best_5_avg_eval.0 as fsize;
            let less_than_5 = n_rollouts < 5;

            RolloutBucket{
                p_opt,
                rollouts,
                avg_eval,
                best_eval,
                best_5_avg_eval,
                less_than_5
            }
        })
        .collect()
}
