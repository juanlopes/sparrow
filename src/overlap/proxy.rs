use crate::config::{OVERLAP_PROXY_EPSILON_DIAM_RATIO, OVERLAP_PROXY_NEGLECT_EPSILON_RATIO};
use jagua_rs::geometry::fail_fast::sp_surrogate::SPSurrogate;
use jagua_rs::geometry::geo_traits::{Distance, Shape};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use std::cmp::Ordering;

#[inline(always)]
pub fn poly_overlap_proxy(s1: &SimplePolygon, s2: &SimplePolygon) -> f32 {
    let epsilon = f32::max(s1.diameter(), s2.diameter()) * OVERLAP_PROXY_EPSILON_DIAM_RATIO;

    let deficit = poles_overlap_proxy(
        &s1.surrogate(),
        &s2.surrogate(),
        epsilon,
    );

    let s1_penalty = s1.surrogate().convex_hull_area;
    let s2_penalty = s2.surrogate().convex_hull_area;

    //let penalty = f32::min(s1_penalty, s2_penalty);
    let penalty = 0.95 * f32::min(s1_penalty, s2_penalty) + 0.05 * f32::max(s1_penalty, s2_penalty);

    (deficit * penalty).sqrt()
}

#[inline(always)]
pub fn bin_overlap_proxy(s: &SimplePolygon, bin_bbox: AARectangle) -> f32 {
    let s_bbox = s.bbox();
    let deficit = match AARectangle::from_intersection(&s_bbox, &bin_bbox) {
        Some(r) => {
            let negative_area = s_bbox.area() - r.area();
            negative_area
        }
        None => {
            //no intersection, guide towards intersection with bin
            s_bbox.area() + s_bbox.centroid().distance(&bin_bbox.centroid())
        }
    };
    let penalty = s.surrogate().convex_hull_area;

    10.0 * (deficit * penalty).sqrt()
}
#[inline(always)]
pub fn poles_overlap_proxy<'a>(sp1: &SPSurrogate, sp2: &SPSurrogate, epsilon: f32) -> f32 {
    let (sp_inner, sp_outer) = choose_inner_outer(sp1, sp2);
    let bpole_inner = sp_inner.poles_bounding_circle.clone();

    let mut total_deficit = 0.0;
    for p1 in &sp_outer.poles {
        //if the pole is far enough outside the bounding circle of the inner surrogate poles, skip it.
        //its deficit will be negligible and this speeds up the calculation quite a bit
        let sq_distance = p1.center.sq_distance(&bpole_inner.center);
        let neglect_sq_dist = (p1.radius + bpole_inner.radius + OVERLAP_PROXY_NEGLECT_EPSILON_RATIO * epsilon).powi(2);
        if sq_distance > neglect_sq_dist {
            continue;
        } else {
            for p2 in &sp_inner.poles {
                let pd = (p1.radius + p2.radius) - p1.center.distance(&p2.center);

                let pd_decay = match pd >= epsilon {
                    true => pd,
                    false => epsilon.powi(2) / (-pd + 2.0 * epsilon),
                };

                total_deficit += pd_decay * f32::min(p1.radius, p2.radius);
            }
        }
    }
    total_deficit
}

fn choose_inner_outer<'a>(sp1: &'a SPSurrogate, sp2: &'a SPSurrogate) -> (&'a SPSurrogate, &'a SPSurrogate) {
    //selects the surrogate with the smaller bounding circle as the inner surrogate
    let bp1 = &sp1.poles_bounding_circle;
    let bp2 = &sp2.poles_bounding_circle;
    match bp1.radius.partial_cmp(&bp2.radius).unwrap(){
        Ordering::Less => (sp1, sp2),
        Ordering::Greater => (sp2, sp1),
        Ordering::Equal => { //tiebreaker to ensure associativity
            match (bp1.center.0 + bp1.center.1).partial_cmp(&(bp2.center.0 + bp2.center.1)).unwrap(){
                Ordering::Less => (sp1, sp2),
                _ => (sp2, sp1),
            }
        }
    }
}