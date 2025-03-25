use jagua_rs::entities::item::Item;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_enums::AllowedRotation;
use jagua_rs::geometry::geo_traits::{Shape, TransformableFrom};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::transformation::Transformation;
use rand::prelude::IndexedRandom;
use rand::Rng;
use std::ops::Range;
use itertools::Itertools;

#[derive(Clone, Debug)]
pub struct UniformBBoxSampler {
    rot_entries: Vec<RotEntry>,
}

#[derive(Clone, Debug)]
struct RotEntry {
    pub r: f32,
    pub x_range: Range<f32>,
    pub y_range: Range<f32>,
}

impl UniformBBoxSampler {
    pub fn new(sample_bbox: AARectangle, item: &Item, bin_bbox: AARectangle) -> Option<Self> {
        let rotations = match &item.allowed_rotation {
            AllowedRotation::None => &vec![0.0],
            AllowedRotation::Discrete(r) => r,
            AllowedRotation::Continuous => unreachable!(),
        };

        let mut buff = item.shape.as_ref().clone();

        let sample_x_range = sample_bbox.x_min..sample_bbox.x_max;
        let sample_y_range = sample_bbox.y_min..sample_bbox.y_max;

        let rot_entries = rotations.iter()
            .map(|&r| {
                let r_shape_bbox = buff.transform_from(item.shape.as_ref(), &Transformation::from_rotation(r)).bbox();
                let abs_x_range = (bin_bbox.x_min - r_shape_bbox.x_min)..(bin_bbox.x_max - r_shape_bbox.x_max);
                let abs_y_range = (bin_bbox.y_min - r_shape_bbox.y_min)..(bin_bbox.y_max - r_shape_bbox.y_max);

                let x_range = abs_x_range.start.max(sample_x_range.start)..abs_x_range.end.min(sample_x_range.end);
                let y_range = abs_y_range.start.max(sample_y_range.start)..abs_y_range.end.min(sample_y_range.end);

                //make sure the ranges are not empty
                if !x_range.is_empty() && !y_range.is_empty() {
                    Some(RotEntry { r, x_range, y_range })
                }
                else{
                    None
                }
            }).flatten().collect_vec();

        match rot_entries.len(){
            0 => None,
            _ => Some(Self { rot_entries }),
        }
    }

    pub fn sample(&self, rng: &mut impl Rng) -> DTransformation {
        let r_entry = self.rot_entries.choose(rng).unwrap();

        let r = r_entry.r;
        let x_sample = rng.random_range(r_entry.x_range.clone());
        let y_sample = rng.random_range(r_entry.y_range.clone());

        DTransformation::new(r, (x_sample, y_sample))
    }
}