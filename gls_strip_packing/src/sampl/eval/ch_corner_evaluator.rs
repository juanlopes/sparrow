use itertools::Itertools;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::fsize;
use jagua_rs::geometry::convex_hull::{convex_hull_from_points, convex_hull_from_shapes};
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_traits::{Shape, TransformableFrom};
use jagua_rs::geometry::primitives::point::Point;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use ordered_float::OrderedFloat;
use crate::sampl::eval::{SampleEval, SampleEvaluator};

pub struct ChCornerEvaluator<'a> {
    layout: &'a Layout,
    item: &'a Item,
    corners: [Point; 4],
    corner_convex_hulls: [Vec<Point>; 4],
    original_ch_areas: [fsize; 4],
    shape_buff: SimplePolygon,
    n_evals: usize,
}

impl<'a> ChCornerEvaluator<'a> {
    pub fn new(layout: &'a Layout, item: &'a Item) -> Self {
        let corners = layout.bin.bbox().corners();
        let mut corner_ch_candidates = corners.clone().map(|c| vec![c]);
        for pi in layout.placed_items.values(){
            //find the closest corner to the centroid
            let centroid = pi.shape.centroid();
            let closest_corner_idx = corners.iter().enumerate()
                .min_by_key(|(_, c)| OrderedFloat(c.distance(centroid)))
                .map(|(i, _)| i)
                .unwrap();
            let convex_hull_points = pi.shape.surrogate().convex_hull_indices.iter().map(|i| pi.shape.points[*i]);
            corner_ch_candidates[closest_corner_idx].extend(convex_hull_points);
        }

        corner_ch_candidates[0].extend(extend_ch_candidates(0, &corner_ch_candidates[0]));
        corner_ch_candidates[1].extend(extend_ch_candidates(1, &corner_ch_candidates[1]));
        corner_ch_candidates[2].extend(extend_ch_candidates(2, &corner_ch_candidates[2]));
        corner_ch_candidates[3].extend(extend_ch_candidates(3, &corner_ch_candidates[3]));

        let corner_convex_hulls = corner_ch_candidates.map(|c| convex_hull_from_points(c));

        let corner_convex_hulls_areas: [fsize;4] = corner_convex_hulls.iter()
            .map(|ch| SimplePolygon::calculate_area(ch))
            .collect::<Vec<fsize>>().try_into().unwrap();

        Self {
            layout,
            item,
            corners: layout.bin.bbox().corners(),
            shape_buff: item.shape.as_ref().clone(),
            corner_convex_hulls,
            original_ch_areas: corner_convex_hulls_areas,
            n_evals: 0,
        }
    }
}

impl<'a> SampleEvaluator for ChCornerEvaluator<'a> {
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
                        //let s_bbox = self.shape_buff.bbox();
                        let closest_corner_idx = self.corners.iter().enumerate()
                            .min_by_key(|(_,c)| OrderedFloat(c.distance(self.shape_buff.centroid())))
                            .map(|(i, _)| i)
                            .unwrap();
                        let mut ch_candidates = self.shape_buff.surrogate().convex_hull_indices.iter().map(|i| self.shape_buff.points[*i])
                            .chain(self.corner_convex_hulls[closest_corner_idx].iter().cloned())
                            .collect_vec();

                        ch_candidates.extend(extend_ch_candidates(closest_corner_idx, &ch_candidates));

                        let ch_points = convex_hull_from_points(ch_candidates);

                        let expanded_ch_hull = SimplePolygon::calculate_area(&ch_points);

                        let ch_area_expansion = expanded_ch_hull - self.original_ch_areas[closest_corner_idx];

                        let min_corner_distance = {
                            self.corners[closest_corner_idx].distance(self.shape_buff.centroid())
                        };

                        let value = ch_area_expansion + 0.01 * min_corner_distance.powi(2);
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
