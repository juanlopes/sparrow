use jagua_rs::fsize;
use jagua_rs::geometry::geo_enums::GeoPosition;
use jagua_rs::geometry::geo_traits::{Distance, SeparationDistance, Shape};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::circle::Circle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use ordered_float::{Float, OrderedFloat};

pub const NORMALIZER_DIAM_FRAC: fsize = 0.005;

pub fn poly_overlap_proxy(s1: &SimplePolygon, s2: &SimplePolygon, bin_bbox: AARectangle) -> fsize {
    let normalizer = fsize::max(s1.diameter(),s2.diameter()) * NORMALIZER_DIAM_FRAC;

    let deficit = poles_overlap_proxy(
        s1.surrogate().poles.iter(),
        s2.surrogate().poles.iter(),
        normalizer,
    );

    let s1_penalty = (s1.surrogate().convex_hull_area); //+ //0.1 * (s1.diameter / 4.0).powi(2));
    let s2_penalty = (s2.surrogate().convex_hull_area); // + 0.1 * (s2.diameter / 4.0).powi(2));

    let penalty = 0.99 * fsize::min(s1_penalty,s2_penalty) + 0.01 * fsize::max(s1_penalty,s2_penalty);

    (deficit + 0.00 * penalty).sqrt() * penalty.sqrt()
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

    10000.0 * (deficit + 0.00 * penalty).sqrt() * penalty.sqrt()
}

pub fn poles_overlap_proxy_old<'a, C>(poles_1: C, poles_2: C, normalizer: fsize) -> fsize
where
    C: Iterator<Item=&'a Circle> + Clone,
{
    let mut deficit = 0.0;
    for p1 in poles_1 {
        for p2 in poles_2.clone() {
            let value = match p1.separation_distance(p2) {
                (GeoPosition::Interior, d) => {
                    d + normalizer
                },
                (GeoPosition::Exterior, d) => normalizer / (d / normalizer + 1.0),
            };

            deficit += value * fsize::min(p1.radius, p2.radius);
        }
    }
    deficit
}

pub fn poles_overlap_proxy<'a, C>(poles_1: C, poles_2: C, epsilon: fsize) -> fsize
where
    C: Iterator<Item=&'a Circle> + Clone,
{
    let mut deficit = 0.0;
    for p1 in poles_1 {
        for p2 in poles_2.clone() {
            let d = (p1.radius + p2.radius) - p1.center.distance(p2.center);
            let d_m = d - epsilon;

            let d_prime = match d_m >= 0.0 {
                true => d_m + epsilon,
                false => epsilon.powi(2) / (-d_m + epsilon),
            };

            deficit += d_prime * fsize::min(p1.radius, p2.radius);
        }
    }
    deficit
}

fn distance_between_bboxes(big_bbox: &AARectangle, small_bbox: &AARectangle) -> fsize {
    let min_d = [big_bbox.x_max - small_bbox.x_max,
        small_bbox.x_min - big_bbox.x_min,
        big_bbox.y_max - small_bbox.y_max,
        small_bbox.y_min - big_bbox.y_min].iter().min_by_key(|d| OrderedFloat(**d)).copied().unwrap();

    assert!(min_d >= -1.0);

    min_d
}
