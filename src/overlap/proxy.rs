use crate::config::OVERLAP_PROXY_EPSILON_DIAM_RATIO;
use jagua_rs::geometry::fail_fast::sp_surrogate::SPSurrogate;
use jagua_rs::geometry::geo_traits::{Distance, Shape};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use std::cmp::Ordering;

/// Evaluates the overlap between two simple polygons.
#[inline(always)]
pub fn eval_overlap_poly_poly(s1: &SimplePolygon, s2: &SimplePolygon) -> f32 {
    let epsilon = f32::max(s1.diameter(), s2.diameter()) * OVERLAP_PROXY_EPSILON_DIAM_RATIO;

    let overlap_proxy = poles_overlap_area_proxy(&s1.surrogate(), &s2.surrogate(), epsilon);

    debug_assert!(overlap_proxy.is_normal());

    let penalty = (s1.surrogate().convex_hull_area * s2.surrogate().convex_hull_area).sqrt();

    (overlap_proxy * penalty).sqrt()
}

/// Evaluates the overlap between a simple polygon and the exterior of the bin.
#[inline(always)]
pub fn eval_overlap_poly_bin(s: &SimplePolygon, bin_bbox: AARectangle) -> f32 {
    let s_bbox = s.bbox();
    let overlap = match AARectangle::from_intersection(&s_bbox, &bin_bbox) {
        Some(r) => {
            //intersection exist, calculate the area of the intersection (+ a small value to ensure it is never zero)
            let negative_area = (s_bbox.area() - r.area()) + 0.001 * s_bbox.area();
            negative_area
        }
        None => {
            //no intersection, guide towards intersection with bin
            s_bbox.area() + s_bbox.centroid().distance(&bin_bbox.centroid())
        }
    };
    debug_assert!(overlap.is_normal());

    let penalty = s.surrogate().convex_hull_area;

    10.0 * (overlap * penalty).sqrt()
}


/// Calculates a proxy for the overlap area between two sets of poles
#[inline(always)]
pub fn poles_overlap_area_proxy<'a>(sp1: &SPSurrogate, sp2: &SPSurrogate, epsilon: f32) -> f32 {
    let (sp_inner, sp_outer) = select_inner_outer(sp1, sp2);
    let bounding_pole_inner = sp_inner.poles_bounding_circle.clone();
    let safety_margin = sp1.max_distance_point_to_pole + sp2.max_distance_point_to_pole;

    let mut total_overlap = 0.0;
    for p_o in &sp_outer.poles {
        // If the pole is far enough outside the bounding circle of the inner surrogate poles, skip it.
        // Its deficit will be negligible and neglecting it saves a lot of time.
        let sq_dist_to_inner_bp = p_o.center.sq_distance(&bounding_pole_inner.center);
        let sq_dist_neglect = (p_o.radius + bounding_pole_inner.radius + safety_margin).powi(2);
        if sq_dist_to_inner_bp > sq_dist_neglect {
            //p_o is far enough away from the inner surrogate's bounding pole
            continue;
        } else {
            // Not far enough away, so we need to check the inner surrogate's poles
            for p_i in &sp_inner.poles {
                // Penetration depth between the two poles (circles)
                let pd = (p_o.radius + p_i.radius) - p_o.center.distance(&p_i.center);

                let pd_decay = match pd >= epsilon {
                    true => pd,
                    false => epsilon.powi(2) / (-pd + 2.0 * epsilon),
                };

                total_overlap += pd_decay * f32::min(p_o.radius, p_i.radius);
            }
        }
    }
    debug_assert!(total_overlap.is_normal());
    total_overlap
}

fn select_inner_outer<'a>(
    sp1: &'a SPSurrogate,
    sp2: &'a SPSurrogate,
) -> (&'a SPSurrogate, &'a SPSurrogate) {
    //selects the surrogate with the smaller bounding circle as the inner surrogate
    let bp1 = &sp1.poles_bounding_circle;
    let bp2 = &sp2.poles_bounding_circle;
    match bp1.radius.partial_cmp(&bp2.radius).unwrap() {
        Ordering::Less => (sp1, sp2),
        Ordering::Greater => (sp2, sp1),
        Ordering::Equal => {
            //In case it is the same item, tiebreaker to ensure associativity of the function
            match (bp1.center.0 + bp1.center.1).partial_cmp(&(bp2.center.0 + bp2.center.1)).unwrap()
            {
                Ordering::Less => (sp1, sp2),
                _ => (sp2, sp1),
            }
        }
    }
}
