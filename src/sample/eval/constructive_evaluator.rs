use crate::sample::eval::{SampleEval, SampleEvaluator};
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::problems::strip_packing;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::{Shape, TransformableFrom};
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;

pub const X_MULTIPLIER: fsize = 10.0;
pub const Y_MULTIPLIER: fsize = 1.0;

pub struct ConstructiveEvaluator<'a> {
    layout: &'a Layout,
    item: &'a Item,
    shape_buff: SimplePolygon,
    n_evals: usize,
    strip_occup: fsize,
}

impl<'a> ConstructiveEvaluator<'a> {
    pub fn new(layout: &'a Layout, item: &'a Item) -> Self {
        let strip_occup = strip_packing::occupied_range(layout).map_or(0.0, |(min, max)| max);
        Self {
            layout,
            item,
            shape_buff: item.shape.as_ref().clone(),
            n_evals: 0,
            strip_occup,
        }
    }
}

impl<'a> SampleEvaluator for ConstructiveEvaluator<'a> {
    fn eval(&mut self, dt: DTransformation, upper_bound: Option<SampleEval>) -> SampleEval {
        self.n_evals += 1;
        let cde = self.layout.cde();
        let t = dt.into();
        let irrel_hazards = &[];
        match cde.surrogate_collides(self.item.shape.surrogate(), &t, irrel_hazards) {
            true => SampleEval::Invalid,
            false => {
                self.shape_buff.transform_from(&self.item.shape, &t);
                match cde.poly_collides(&self.shape_buff, irrel_hazards) {
                    true => SampleEval::Invalid,
                    false => {
                        let poi = self.shape_buff.poi.center;
                        let poi_eval = X_MULTIPLIER * poi.0 + Y_MULTIPLIER * poi.1;
                        let bbox_eval =
                            self.shape_buff.bbox().x_max + 0.1 * self.shape_buff.bbox().y_max;
                        SampleEval::Valid(poi_eval + bbox_eval)
                    }
                }
            }
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}
