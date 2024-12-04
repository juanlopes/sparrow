use itertools::Itertools;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::fsize;
use jagua_rs::geometry::convex_hull::{convex_hull_from_points, convex_hull_from_shapes};
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_enums::GeoRelation;
use jagua_rs::geometry::geo_traits::{Shape, TransformableFrom};
use jagua_rs::geometry::primitives::edge::Edge;
use jagua_rs::geometry::primitives::point::Point;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use ordered_float::OrderedFloat;
use tap::Tap;
use crate::sampl::eval::{SampleEval, SampleEvaluator};

pub struct ChEdgeEvaluator<'a> {
    layout: &'a Layout,
    item: &'a Item,
    edge_base_chs: [Vec<Point>; 4],
    shape_buff: SimplePolygon,
    n_evals: usize,
}

impl<'a> ChEdgeEvaluator<'a> {
    pub fn new(layout: &'a Layout, item: &'a Item) -> Self {
        let edges = [layout.bin.bbox().top_edge(), layout.bin.bbox().right_edge(), layout.bin.bbox().bottom_edge(), layout.bin.bbox().left_edge()];
        let bin_bbox = layout.bin.bbox();
        let bin_c = bin_bbox.centroid();
        let regions = [
            bin_bbox.clone().tap_mut(|b| b.y_min = bin_c.y()),
            bin_bbox.clone().tap_mut(|b| b.x_min = bin_c.x()),
            bin_bbox.clone().tap_mut(|b| b.y_max = bin_c.y()),
            bin_bbox.clone().tap_mut(|b| b.x_max = bin_c.x()),
        ];

        let edge_base_chs = [0,1,2,3].map(|i| {
            let edge = &edges[i];
            let region = &regions[i];
            let mut edge_base_ch = vec![edge.start, edge.end];
            for pi in layout.placed_items.values(){
                if pi.shape.bbox.almost_relation_to(region) == GeoRelation::Enclosed {
                    let convex_hull_points =
                        pi.shape.surrogate().convex_hull_indices.iter().map(|i| pi.shape.points[*i]);
                    edge_base_ch.extend(convex_hull_points);
                }
            }
            convex_hull_from_points(edge_base_ch)
        });

        let shape_buff = item.shape.as_ref().clone();

        Self {
            layout,
            item,
            edge_base_chs,
            shape_buff,
            n_evals: 0,
        }
    }
}

impl<'a> SampleEvaluator for ChEdgeEvaluator<'a> {
    fn eval(&mut self, dt: DTransformation) -> SampleEval {
        self.n_evals += 1;
        let cde = self.layout.cde();

        let t = dt.into();

        match cde.surrogate_collides(self.item.shape.surrogate(), &t, &[]){
            true => SampleEval::Colliding(usize::MAX, fsize::INFINITY),
            false => {
                self.shape_buff.transform_from(&self.item.shape, &t);
                match cde.poly_collides(&self.shape_buff, &[]) {
                    true => SampleEval::Colliding(usize::MAX, fsize::INFINITY),
                    false => {
                        let shape_ch_points = self.shape_buff.surrogate().convex_hull_indices.iter().map(|i| self.shape_buff.points[*i]);

                        let ch_edge_areas = self.edge_base_chs.clone()
                            .map(|mut base_ch| {
                                base_ch.extend(shape_ch_points.clone());
                                let convex_hull = convex_hull_from_points(base_ch);
                                SimplePolygon::calculate_area(&convex_hull)
                            });

                        let value = ch_edge_areas.iter()
                            .k_smallest_by_key(2, |&&x| OrderedFloat(x))
                            .product();

                        SampleEval::Valid(value)
                    }
                }
            }
        }
    }

    fn n_evals(&self) -> usize {
        self.n_evals
    }
}

fn extend_ch_candidates(corner_idx: usize, points: &[Point]) -> [Point;3] {
    let b = SimplePolygon::generate_bounding_box(&points).corners();
    match corner_idx {
        0 => [b[0], b[1], b[2]],
        1 => [b[1], b[0], b[3]],
        2 => [b[2], b[0], b[3]],
        3 => [b[3], b[1], b[2]],
        _ => unreachable!()
    }
}