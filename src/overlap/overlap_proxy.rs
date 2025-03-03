use crate::config::OVERLAP_PROXY_EPSILON_DIAM_RATIO;
use jagua_rs::fsize;
use jagua_rs::geometry::fail_fast::sp_surrogate::SPSurrogate;
use jagua_rs::geometry::geo_traits::{Distance, Shape};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::circle::Circle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use std::cell::RefCell;
use std::cmp::Ordering;

pub fn poly_overlap_proxy(s1: &SimplePolygon, s2: &SimplePolygon) -> fsize {
    let normalizer = fsize::max(s1.diameter(), s2.diameter()) * OVERLAP_PROXY_EPSILON_DIAM_RATIO;

    let deficit = poles_overlap_proxy(
        &s1.surrogate(),
        &s2.surrogate(),
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
            s_bbox.area() + s_bbox.centroid().distance(&bin_bbox.centroid())
        }
    };
    let penalty = s.surrogate().convex_hull_area;

    10.0 * (deficit + 0.001 * penalty).sqrt() * penalty.sqrt()
}

pub fn poles_overlap_proxy_<'a>(sp1: &SPSurrogate, sp2: &SPSurrogate, epsilon: fsize) -> fsize {
    let epsilon_squared = epsilon.powi(2);
    let two_epsilon = 2.0 * epsilon;

    sp1.poles.iter().flat_map(|p1| {
        sp2.poles.iter().map(move |p2| {
            let pd = (p1.radius + p2.radius) - p1.center.distance(&p2.center);
            let pd_decay = if pd >= epsilon {
                pd
            } else {
                epsilon_squared / (-pd + two_epsilon)
            };
            pd_decay * fsize::min(p1.radius, p2.radius)
        })
    }).sum()
}

pub fn poles_overlap_proxy<'a>(sp1: &SPSurrogate, sp2: &SPSurrogate, epsilon: fsize) -> fsize {
    let (sp_inner, sp_outer) = determine_inner_outer(sp1, sp2);
    let bpole_inner = sp_inner.poles_bounding_circle.clone();

    let mut total_deficit = 0.0;
    for p1 in &sp_outer.poles {
        let sq_distance = p1.center.sq_distance(&bpole_inner.center);
        if sq_distance > (p1.radius + bpole_inner.radius + 5.0 * epsilon).powi(2) {
            //if the pole is outside the bounding circle of the inner surrogate poles, skip it
            //its deficit will be negligible
            continue;
        }
        else {
            for p2 in &sp_inner.poles{
                let pd = (p1.radius + p2.radius) - p1.center.distance(&p2.center);

                let pd_decay = match pd >= epsilon {
                    true => pd,
                    false => epsilon.powi(2) / (-pd + 2.0 * epsilon),
                };

                total_deficit += pd_decay * fsize::min(p1.radius, p2.radius);
            }
        }
    }
    total_deficit
}

fn determine_inner_outer<'a>(sp1: &'a SPSurrogate, sp2: &'a SPSurrogate) -> (&'a SPSurrogate, &'a SPSurrogate) {
    let bp1 = &sp1.poles_bounding_circle;
    let bp2 = &sp2.poles_bounding_circle;
    let cmp = bp1.radius.partial_cmp(&bp2.radius).unwrap()
        .then((bp1.center.0 + bp1.center.1).partial_cmp(&(bp2.center.0 + bp2.center.1)).unwrap());
    match cmp {
        Ordering::Less => (sp1, sp2),
        _ => (sp2, sp1),
    }
}