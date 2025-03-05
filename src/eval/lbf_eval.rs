use crate::eval::sample_eval::{SampleEval, SampleEvaluator};
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::{Shape, TransformableFrom};
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;

pub const X_MULTIPLIER: f32 = 10.0;
pub const Y_MULTIPLIER: f32 = 1.0;

pub struct LBFEvaluator<'a> {
    layout: &'a Layout,
    item: &'a Item,
    shape_buff: SimplePolygon,
    n_evals: usize
}

impl<'a> LBFEvaluator<'a> {
    pub fn new(layout: &'a Layout, item: &'a Item) -> Self {
        Self {
            layout,
            item,
            shape_buff: item.shape.as_ref().clone(),
            n_evals: 0
        }
    }
}

impl<'a> SampleEvaluator for LBFEvaluator<'a> {
    fn eval(&mut self, dt: DTransformation, _upper_bound: Option<SampleEval>) -> SampleEval {
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
                        //no collisions
                        let poi = self.shape_buff.poi.center;
                        let poi_eval = X_MULTIPLIER * poi.0 + Y_MULTIPLIER * poi.1;
                        let bbox_eval =
                            self.shape_buff.bbox().x_max + 0.1 * self.shape_buff.bbox().y_max;
                        SampleEval::Clear{loss: poi_eval + bbox_eval}
                    }
                }
            }
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}
