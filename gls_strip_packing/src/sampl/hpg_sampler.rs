use itertools::Itertools;
use jagua_rs::collision_detection::hpg::hpg_cell::HPGCell;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::fsize;
use jagua_rs::geometry::geo_traits::{CollidesWith, Shape};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::transformation::Transformation;
use log::debug;
use rand::distributions::uniform::UniformSampler;
use rand::distributions::{Distribution, Uniform};
use rand::prelude::{SliceRandom, SmallRng};
use rand::Rng;
use crate::sampl::uniform_sampler::UniformAARectSampler;

/// Creates `Transformation` samples for a given item.
/// Samples from the Hazard Proximity Grid uniformly, but only cells which could accommodate the item.
/// Cells were a collision is guaranteed are discarded.
#[derive(Debug, Clone)]
pub struct HPGSampler<'a> {
    pub item: &'a Item,
    pub cell_samplers: Vec<UniformAARectSampler>,
    pub pretransform: Transformation,
    pub bin_bbox_area: fsize,
}

impl<'a> HPGSampler<'a> {
    pub fn new(item: &'a Item, layout: &Layout, limited_bbox: Option<AARectangle>, poi_slack: Option<fsize>) -> Option<HPGSampler<'a>> {
        let poi = &item.shape.poi;
        let bin_bbox = layout.bin.bbox();

        //create a pre-transformation which centers the shape around its Pole of Inaccessibility.
        let pretransform = Transformation::from_translation((-poi.center.0, -poi.center.1));

        let hpg = layout.cde().haz_prox_grid().unwrap();
        let all_cells = hpg.grid.cells.iter().flatten();
        let eligible_cells = all_cells
            .filter(|c| limited_bbox.as_ref().map_or(true, |bbox| bbox.collides_with(&c.bbox)))
            .filter(|c| HPGSampler::cell_could_accommodate_item_with_slack(c, item, poi_slack.unwrap_or(0.0)));

        //create samplers for all eligible cells
        let cell_samplers = eligible_cells
            .filter_map(|c| {
                //map each eligible cell to a rectangle sampler, bounded by the layout's bbox.
                //(at low densities, the cells could extend significantly beyond the layout's bbox)
                AARectangle::from_intersection(&c.bbox, &bin_bbox)
            })
            .map(|bbox| UniformAARectSampler::new(bbox, item))
            .collect_vec();

        match cell_samplers.is_empty() {
            true => {
                debug!("[HPG] no eligible cells to sample from");
                None
            }
            false => {
                debug!(
                    "[HPGS] created sampler with {} eligible cells out of {}",
                    cell_samplers.len(),
                    hpg.grid.n_elements
                );
                Some(HPGSampler {
                    item,
                    cell_samplers,
                    pretransform,
                    bin_bbox_area: bin_bbox.area(),
                })
            }
        }
    }

    /// Samples a `Transformation`
    pub fn sample(&self, rng: &mut impl Rng) -> Transformation {
        //sample one of the eligible cells
        let cell_sampler = self.cell_samplers.choose(rng).expect("no active samplers");

        //from that cell, sample a transformation
        let sample = cell_sampler.sample(rng);

        //combine the pretransform with the sampled transformation
        self.pretransform.clone().transform_from_decomposed(&sample)
    }

    fn cell_could_accommodate_item_with_slack(cell: &HPGCell, item: &Item, slack: fsize) -> bool {
        let poi_d = item.shape.poi.radius - slack;
        if cell.radius > poi_d {
            //impossible to give any guarantees if the cell radius is larger than the Item's POI
            true
        } else {
            //distance of closest relevant hazard
            let haz_prox = cell.hazard_proximity(item.base_quality);

            poi_d < haz_prox + cell.radius
        }
    }
}
