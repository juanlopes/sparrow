use std::simd::{f32x4, StdFloat};
use std::simd::prelude::{SimdFloat, SimdPartialOrd};
use float_cmp::{approx_eq};
use jagua_rs::geometry::fail_fast::sp_surrogate::SPSurrogate;
use jagua_rs::geometry::geo_traits::{Distance, Shape};
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use crate::config::OVERLAP_PROXY_EPSILON_DIAM_RATIO;
use crate::overlap::simd::circles_soa::CirclesSoA;

#[inline(always)]
pub fn eval_overlap_poly_poly_simd(s1: &SimplePolygon, s2: &SimplePolygon, poles2: &CirclesSoA) -> f32 {
    let epsilon = f32::max(s1.diameter(), s2.diameter()) * OVERLAP_PROXY_EPSILON_DIAM_RATIO;

    let overlap_proxy = poles_overlap_area_proxy_simd(&s1.surrogate(), &s2.surrogate(), epsilon, poles2);

    debug_assert!(overlap_proxy.is_normal());

    let penalty = (s1.surrogate().convex_hull_area * s2.surrogate().convex_hull_area).sqrt();

    (overlap_proxy * penalty).sqrt()
}


#[inline(always)]
pub fn poles_overlap_area_proxy_simd(sp1: &SPSurrogate, sp2: &SPSurrogate, epsilon: f32, poles2: &CirclesSoA) -> f32 {
    debug_assert!({
        //check if poles in poles2 equal sp2.poles()
        let result = sp2.poles.iter().enumerate()
            .all(|(i, p)| {
                let x = poles2.x[i];
                let y = poles2.y[i];
                let r = poles2.r[i];
                approx_eq!(f32, p.center.x(), x, epsilon = 0.0001 * p.center.x()) &&
                approx_eq!(f32, p.center.y(), y, epsilon = 0.0001 * p.center.y()) &&
                approx_eq!(f32, p.radius, r, epsilon = 0.0001 * p.radius)
            });

        if !result {
            dbg!(&sp2.poles);
            dbg!(&poles2);
        }
        result
    }, "poles2 does not match sp2.poles()");

    let epsilon_splat_4 = f32x4::splat(epsilon);
    let epsilon_squared_4 = f32x4::splat(epsilon * epsilon);
    let two_epsilon_4 = f32x4::splat(2.0 * epsilon);

    let bounding_pole_inner = sp2.poles_bounding_circle.clone();
    let safety_margin = sp1.max_distance_point_to_pole + sp2.max_distance_point_to_pole;

    let mut total_overlap = 0.0;
    for p_o in sp1.poles.iter() {
        // If the pole is far enough outside the bounding circle of the inner surrogate poles, skip it.
        // Its deficit will be negligible and neglecting it saves a lot of time.
        let sq_dist_to_inner_bp = p_o.center.sq_distance(&bounding_pole_inner.center);
        let sq_dist_neglect = (p_o.radius + bounding_pole_inner.radius + safety_margin).powi(2);
        if sq_dist_to_inner_bp > sq_dist_neglect {
            //p_o is far enough away from the inner surrogate's bounding pole
            continue;
        }

        // Process with SIMD if we didn't skip
        // Common values for all chunks
        let r1 = p_o.radius;
        let x1_4 = f32x4::splat(p_o.center.x());
        let y1_4 = f32x4::splat(p_o.center.y());
        let r1_splat_4 = f32x4::splat(r1);

        // Process complete chunks of 4 elements with SIMD
        let chunks = poles2.x.len() / 4;

        for chunk in 0..chunks {
            let start_idx = chunk * 4;

            let x2 = f32x4::from_slice(&poles2.x[start_idx..start_idx + 4]);
            let y2 = f32x4::from_slice(&poles2.y[start_idx..start_idx + 4]);
            let r2 = f32x4::from_slice(&poles2.r[start_idx..start_idx + 4]);

            // calculate penetration depth for 4 pairs at once
            let dx = x1_4 - x2;
            let dy = y1_4 - y2;
            let dist = (dx * dx + dy * dy).sqrt();
            let radius_sum = r1_splat_4 + r2;
            let pd = radius_sum - dist;

            // calculate decayed pd for 4 pairs at once
            let pd_mask = pd.simd_ge(epsilon_splat_4);
            let denominator = -pd + two_epsilon_4;
            let decay_values = epsilon_squared_4 / denominator;
            let pd_decay = pd_mask.select(pd, decay_values);

            // calculate min radius for 4 pairs at once
            let min_r = r1_splat_4.simd_min(r2);

            total_overlap += (pd_decay * min_r).reduce_sum();
        }

        // Process remaining elements (0-3) with scalar operations
        let remaining_start = chunks * 4;
        for j in remaining_start..poles2.x.len() {
            let dx = p_o.center.x() - poles2.x[j];
            let dy = p_o.center.y() - poles2.y[j];
            let dist = (dx * dx + dy * dy).sqrt();

            let pd = r1 + poles2.r[j] - dist;
            let pd_decay = if pd >= epsilon {
                pd
            } else {
                epsilon.powi(2) / (-pd + 2.0 * epsilon)
            };

            total_overlap += pd_decay * r1.min(poles2.r[j]);
        }
    }

    debug_assert!(approx_eq!(f32, total_overlap, poles_overlap_area_proxy_seq(sp1, sp2, epsilon), epsilon = total_overlap * 1e-6), "SIMD and SEQ results do not match: {} vs {}", total_overlap, poles_overlap_area_proxy_seq(sp1, sp2, epsilon));

    debug_assert!(total_overlap.is_normal());
    total_overlap
}


/// Sequential version of the poles_overlap_area_proxy function
fn poles_overlap_area_proxy_seq<'a>(sp1: &SPSurrogate, sp2: &SPSurrogate, epsilon: f32) -> f32 {
    let (sp_inner, sp_outer) = (sp2, sp1);
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
    total_overlap
}


