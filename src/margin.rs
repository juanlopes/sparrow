//! Minimum separation as a true outward offset of each item contour.
//!
//! jagua-rs inflates hazards with `geo_buffer::buffer_polygon_rounded`, a
//! straight-skeleton buffer. On concave contours that buffer self-intersects
//! and its boundary falls inside the requested distance — sometimes by most of
//! the margin (a stepped polygon offset by 2 was only 1 unit clear; a spiral
//! offset by 0.6 self-intersected and was 0.07 clear). Collision checks only
//! see that polygon, so originals can intersect even when the layout is reported
//! as separated.
//!
//! Collision shapes are rebuilt here with `geo`'s buffer, which resolves
//! self-intersections. Round joins are expanded just enough that every chord
//! stays outside the requested radius, and the result is rejected unless the
//! original contour is inside by at least that radius.

use anyhow::{Context, Result, bail};
use geo::algorithm::buffer::{BufferStyle, LineJoin};
use geo::{Buffer, LineString, Polygon};
use jagua_rs::collision_detection::CDEConfig;
use jagua_rs::geometry::geo_enums::GeoPosition;
use jagua_rs::geometry::geo_traits::{SeparationDistance, Transformable};
use jagua_rs::geometry::primitives::{Point, SPolygon};
use jagua_rs::geometry::shape_modification::{ShapeModifyMode, simplify_shape};
use jagua_rs::io::import::Importer;
use jagua_rs::probs::spp::entities::SPInstance;
use jagua_rs::probs::spp::io::ext_repr::ExtSPInstance;
use std::sync::Arc;

/// `LineJoin::Round(a)` limits each arc chord to length `a * radius`.
const JOIN_CHORD_PER_RADIUS: f64 = 0.1;

/// Build an internal instance from the external JSON model.
///
/// `min_item_separation` is the required gap between original contours, in the
/// same units as the input coordinates. `None` or a non-positive value disables it.
pub fn import_spp_instance(
    cde_config: CDEConfig,
    poly_simpl_tolerance: Option<f32>,
    min_item_separation: Option<f32>,
    ext: &ExtSPInstance,
) -> Result<SPInstance> {
    let separation = min_item_separation.filter(|d| d.is_finite() && *d > 0.0);
    // jagua-rs would offset with the straight-skeleton buffer. Keep that off
    // and, when a margin is requested, also skip its pre-offset simplification
    // so a narrow feature is not dropped before the margin is applied.
    let importer = Importer::new(
        cde_config,
        if separation.is_some() {
            None
        } else {
            poly_simpl_tolerance
        },
        None,
    );
    let mut instance = jagua_rs::probs::spp::io::import(&importer, ext)?;

    if let Some(separation) = separation {
        // Each hazard grows by half the gap, matching jagua-rs: two inflated
        // contours (or an inflated item and a deflated strip) meet at the full distance.
        let half = separation / 2.0;
        for (item, _) in &mut instance.items {
            let centered = item
                .shape_orig
                .shape
                .transform_clone(&item.shape_orig.pre_transform.compose());
            let mut collision = outward_offset(&centered, half)
                .with_context(|| format!("failed to offset item {} by {half}", item.id))?;
            if let Some(tolerance) = poly_simpl_tolerance {
                let simplified = simplify_shape(&collision, ShapeModifyMode::Inflate, tolerance);
                if clearance_covers(&centered, &simplified, half) {
                    collision = simplified;
                }
            }
            collision
                .generate_surrogate(item.surrogate_config)
                .with_context(|| format!("failed to build surrogate for item {}", item.id))?;
            item.shape_cd = Arc::new(collision);
        }
        // Rectangle inset. For a convex rectangle the exact inward offset is a
        // smaller sharp rectangle, which is what the strip constructor produces.
        instance.base_strip.shape_modify_config.offset = Some(half);
        instance.base_strip.shape_modify_config.simplify_tolerance = None;
    }

    Ok(instance)
}

/// Radius whose round-join chords stay at least `distance` from the source corner.
fn outer_radius(distance: f32) -> f64 {
    let half_chord = JOIN_CHORD_PER_RADIUS * 0.5;
    let cos_half = (1.0 - half_chord * half_chord).sqrt();
    // Small pad absorbs f32 rounding when the ring is stored for collision.
    f64::from(distance) / cos_half * 1.0005
}

fn outward_offset(shape: &SPolygon, distance: f32) -> Result<SPolygon> {
    let mut radius = outer_radius(distance);
    let mut failure = String::from("no attempt");
    for _ in 0..4 {
        match buffer_polygon(shape, radius) {
            Ok(offset) => {
                let clearance = contour_clearance(shape, &offset);
                if clearance_covers(shape, &offset, distance) {
                    return Ok(offset);
                }
                let scale = if clearance > distance * 0.05 {
                    f64::from(distance / clearance) * 1.02
                } else {
                    1.5
                };
                radius *= scale;
                failure = format!("clearance {clearance:.6} < {distance:.6}");
            }
            Err(err) => {
                radius *= 1.05;
                failure = err.to_string();
            }
        }
    }
    bail!("outward offset by {distance} failed ({failure})")
}

fn clearance_covers(inner: &SPolygon, outer: &SPolygon, distance: f32) -> bool {
    let slack = distance * 1.0e-4 + 1.0e-4;
    contour_clearance(inner, outer) + slack >= distance
}

/// Signed distance from `inner`'s contour to `outer`'s boundary.
/// Positive when `inner` lies inside `outer`.
fn contour_clearance(inner: &SPolygon, outer: &SPolygon) -> f32 {
    let mut min_signed = f32::MAX;
    for edge in inner.edge_iter() {
        let length = (edge.end.0 - edge.start.0).hypot(edge.end.1 - edge.start.1);
        let steps = ((length / 0.25).ceil() as usize).clamp(8, 64);
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let point = Point(
                edge.start.0 + (edge.end.0 - edge.start.0) * t,
                edge.start.1 + (edge.end.1 - edge.start.1) * t,
            );
            let (position, distance) = outer.separation_distance(&point);
            let signed = match position {
                GeoPosition::Interior => distance,
                GeoPosition::Exterior => -distance,
            };
            min_signed = min_signed.min(signed);
        }
    }
    min_signed
}

fn buffer_polygon(shape: &SPolygon, radius: f64) -> Result<SPolygon> {
    let mut coords: Vec<(f64, f64)> = shape
        .vertices
        .iter()
        .map(|point| (f64::from(point.0), f64::from(point.1)))
        .collect();
    if coords.len() < 3 {
        bail!("cannot offset a polygon with fewer than 3 vertices");
    }
    coords.push(coords[0]);
    let polygon = Polygon::new(LineString::from(coords), vec![]);
    let style = BufferStyle::new(radius).line_join(LineJoin::Round(JOIN_CHORD_PER_RADIUS));
    let buffered = polygon.buffer_with_style(style);

    let mut best: Option<(f32, SPolygon)> = None;
    for part in &buffered.0 {
        let Some(candidate) = ring_to_spolygon(part.exterior()) else {
            continue;
        };
        let clearance = contour_clearance(shape, &candidate);
        let replace = best
            .as_ref()
            .is_none_or(|(previous, _)| clearance > *previous);
        if replace {
            best = Some((clearance, candidate));
        }
    }
    best.map(|(_, polygon)| polygon)
        .context("offset did not produce a simple polygon")
}

fn ring_to_spolygon(ring: &geo::LineString<f64>) -> Option<SPolygon> {
    let mut points: Vec<Point> = Vec::with_capacity(ring.0.len());
    for coord in &ring.0 {
        let point = Point(coord.x as f32, coord.y as f32);
        if !point.0.is_finite() || !point.1.is_finite() {
            return None;
        }
        if let Some(previous) = points.last()
            && (previous.0 - point.0).hypot(previous.1 - point.1) < 1.0e-5
        {
            continue;
        }
        // Exact repeats (not necessarily consecutive after rounding) make SPolygon::new fail.
        if points
            .iter()
            .any(|existing| existing.0 == point.0 && existing.1 == point.1)
        {
            continue;
        }
        points.push(point);
    }
    if points.len() >= 2 {
        let first = points[0];
        let last = *points.last().unwrap();
        if (first.0 - last.0).hypot(first.1 - last.1) < 1.0e-5 {
            points.pop();
        }
    }
    SPolygon::new(points).ok()
}
