use itertools::izip;
use jagua_rs::geometry::geo_traits::{Transformable};
use jagua_rs::geometry::primitives::circle::Circle;
use jagua_rs::geometry::primitives::point::Point;
use jagua_rs::geometry::transformation::Transformation;

/// Collection of circles, but with a memory layout that's more suitable for SIMD operations.
/// SoA (Structure of Arrays) instead of AoS (Array of Structures).

#[derive(Debug, Clone)]
#[repr(align(32))]
pub struct CirclesSoA {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub r: Vec<f32>,
}

impl CirclesSoA {
    pub fn new() -> Self {
        Self {
            x: Vec::new(),
            y: Vec::new(),
            r: Vec::new(),
        }
    }
    pub fn transform_from(&mut self, reference: &Vec<Circle>, t: &Transformation) -> &mut Self {
        self.x.resize(reference.len(), 0.0);
        self.y.resize(reference.len(), 0.0);
        self.r.resize(reference.len(), 0.0);

        //transform poles
        izip!(self.x.iter_mut(), self.y.iter_mut(), self.r.iter_mut())
            .zip(reference.iter())
            .for_each(|((x,y,r),ref_c)| {
                let Point(xt, yt) = ref_c.center.transform_clone(t);
                *x = xt;
                *y = yt;
                *r = ref_c.radius;
            });

        self
    }
}
