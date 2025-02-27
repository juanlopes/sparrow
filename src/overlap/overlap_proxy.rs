use jagua_rs::fsize;
use jagua_rs::geometry::geo_traits::Shape;
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::circle::Circle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use crate::config::PROXY_EPSILON_DIAM_FRAC;

pub fn poly_overlap_proxy(s1: &SimplePolygon, s2: &SimplePolygon) -> fsize {
    let normalizer = fsize::max(s1.diameter(), s2.diameter()) * PROXY_EPSILON_DIAM_FRAC;

    let deficit = poles_overlap_proxy(
        s1.surrogate().poles.iter(),
        s2.surrogate().poles.iter(),
        normalizer,
    );

    let s1_penalty = s1.surrogate().convex_hull_area;
    let s2_penalty = s2.surrogate().convex_hull_area;

    let penalty =
        0.99 * fsize::min(s1_penalty, s2_penalty) + 0.01 * fsize::max(s1_penalty, s2_penalty);

    (deficit + 0.001 * penalty).sqrt() * penalty.sqrt()
}

pub fn bin_overlap_proxy(s: &SimplePolygon, bin_bbox: AARectangle) -> fsize {
    let s_bbox = s.bbox();
    let deficit = match AARectangle::from_intersection(&s_bbox, &bin_bbox) {
        Some(r) => {
            let negative_area = s_bbox.area() - r.area();
            negative_area
        }
        None => {
            //no intersection, guide towards intersection with bin
            s_bbox.area() + s_bbox.centroid().distance(bin_bbox.centroid())
        }
    };
    let penalty = s.surrogate().convex_hull_area;

    10.0 * (deficit + 0.001 * penalty).sqrt() * penalty.sqrt()
}

pub fn poles_overlap_proxy<'a, C>(poles_1: C, poles_2: C, epsilon: fsize) -> fsize
where
    C: Iterator<Item = &'a Circle> + Clone,
{
    let mut total_deficit = 0.0;
    for p1 in poles_1 {
        for p2 in poles_2.clone() {
            let d = (p1.radius + p2.radius) - p1.center.distance(p2.center);

            let d_decay = match d >= epsilon {
                true => d,
                false => epsilon.powi(2) / (-d + 2.0 * epsilon),
            };

            total_deficit += d_decay * fsize::min(p1.radius, p2.radius);
        }
    }
    total_deficit
}
