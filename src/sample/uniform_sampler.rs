use jagua_rs::entities::item::Item;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_enums::AllowedRotation;
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use rand::prelude::IndexedRandom;
use rand::Rng;
use std::f32::consts::PI;
use std::ops::Range;


/// Generates random samples uniformly within a bounding box.
#[derive(Clone, Debug)]
pub struct UniformBBoxSampler {
    pub uniform_x: Range<f32>,
    pub uniform_y: Range<f32>,
    pub uniform_r: UniformRotDistr,
}

impl UniformBBoxSampler {
    pub fn new(bbox: AARectangle, item: &Item) -> Self {
        let uniform_x = bbox.x_min..bbox.x_max;
        let uniform_y = bbox.y_min..bbox.y_max;
        let uniform_r = UniformRotDistr::from_item(item);
        Self {
            uniform_x,
            uniform_y,
            uniform_r,
        }
    }

    pub fn sample(&self, rng: &mut impl Rng) -> DTransformation {
        let r_sample = self.uniform_r.sample(rng);
        let x_sample = rng.random_range(self.uniform_x.clone());
        let y_sample = rng.random_range(self.uniform_y.clone());

        DTransformation::new(r_sample, (x_sample, y_sample))
    }
}

#[derive(Debug, Clone)]
pub enum UniformRotDistr {
    Range(Range<f32>),
    Discrete(Vec<f32>),
    None,
}

impl UniformRotDistr {
    pub fn from_item(item: &Item) -> Self {
        match &item.allowed_rotation {
            AllowedRotation::None => UniformRotDistr::None,
            AllowedRotation::Continuous => UniformRotDistr::Range(0.0..2.0 * PI),
            AllowedRotation::Discrete(a_o) => UniformRotDistr::Discrete(a_o.clone()),
        }
    }

    pub fn sample(&self, rng: &mut impl Rng) -> f32 {
        match self {
            UniformRotDistr::None => 0.0,
            UniformRotDistr::Range(u) => rng.random_range(u.clone()),
            UniformRotDistr::Discrete(a_o) => *a_o.choose(rng).unwrap(),
        }
    }
}
