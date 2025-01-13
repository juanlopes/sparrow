use jagua_rs::collision_detection::hpg::hpg_cell::HPGCell;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::fsize;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::transformation::Transformation;
use rand::distributions::uniform::UniformSampler;
use rand::distributions::{Distribution, WeightedIndex};
use rand::prelude::SliceRandom;
use rand::Rng;
use crate::sample::uniform_sampler::UniformAARectSampler;

/// Creates `Transformation` samples for a given item.
/// Samples from the Hazard Proximity Grid uniformly, but only cells which could accommodate the item.
/// Cells were a collision is guaranteed are discarded.
#[derive(Debug, Clone)]
pub struct HPGBiasedSampler<'a> {
    pub item: &'a Item,
    pub cell_samplers: Vec<(UniformAARectSampler, fsize)>,
    pub weights: WeightedIndex<fsize>,
    pub pretransform: Transformation,
}

impl<'a> HPGBiasedSampler<'a> {
    pub fn new(item: &'a Item, layout: &Layout) -> HPGBiasedSampler<'a> {
        let poi = &item.shape.poi;
        let bin_bbox = layout.bin.bbox();

        //create a pre-transformation which centers the shape around its Pole of Inaccessibility.
        let pretransform = Transformation::from_translation((-poi.center.0, -poi.center.1));

        let hpg = layout.cde().haz_prox_grid().unwrap();
        let mut cell_samplers = Vec::with_capacity(hpg.grid.n_cols * hpg.grid.n_rows);

        //create samplers for all eligible cells
        hpg.grid.cells.iter().flatten()
            .filter_map(|c| {
                match AARectangle::from_intersection(&c.bbox, &bin_bbox) {
                    Some(bbox) => {
                        match bbox.area() > c.bbox.area() * 0.01 {
                            true => Some((UniformAARectSampler::new(bbox, item), HPGBiasedSampler::cell_weight(c, item))),
                            false => None
                        }
                    }
                    None => None
                }
            })
            .for_each(|(sampler, w)| cell_samplers.push((sampler, w)));

        let weights = WeightedIndex::new(cell_samplers.iter().map(|(_, w)| *w)).expect("no active samplers");

        HPGBiasedSampler {
            item,
            cell_samplers,
            weights,
            pretransform,
        }
    }

    /// Samples a `Transformation`
    pub fn sample(&self, rng: &mut impl Rng) -> Transformation {
        //sample one of the eligible cells
        let cell_sampler = &self.cell_samplers[self.weights.sample(rng)].0;
        //from that cell, sample a transformation
        let sample = cell_sampler.sample(rng);

        //combine the pretransform with the sampled transformation
        self.pretransform.clone().transform_from_decomposed(&sample)
    }

    fn cell_weight(cell: &HPGCell, item: &Item) -> fsize {
        let haz_prox = cell.hazard_proximity(item.base_quality);
        let value = fsize::min(haz_prox + cell.radius, item.shape.diameter());
        value.powi(2)
    }
}
