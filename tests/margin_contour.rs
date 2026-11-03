//! Irregular contours must keep the requested gap everywhere, not only where a
//! surrogate or a straight-skeleton buffer happens to be accurate.
//!
//! Ported from the pre-1.x margin regression (concave step and spiral).

use jagua_rs::geometry::geo_enums::GeoPosition;
use jagua_rs::geometry::geo_traits::{CollidesWith, DistanceTo, SeparationDistance, Transformable};
use jagua_rs::geometry::primitives::{Point, SPolygon};
use jagua_rs::io::export::int_to_ext_transformation;
use jagua_rs::io::ext_repr::{ExtItem, ExtOrientation, ExtRotation, ExtSPolygon, ExtShape};
use jagua_rs::io::import::Importer;
use jagua_rs::probs::spp::entities::SPInstance;
use jagua_rs::probs::spp::io::ext_repr::{ExtItem as ExtDemandItem, ExtSPInstance};
use rand::SeedableRng;
use rand::rngs::Xoshiro256PlusPlus;
use sparrow::config::DEFAULT_SPARROW_CONFIG;
use sparrow::optimizer::lbf::LBFBuilder;
use sparrow::sample::search::SampleConfig;

/// Concave step. A straight-skeleton offset of 2 collapses to clearance 1 here.
fn step_polygon() -> Vec<(f32, f32)> {
    vec![
        (0.0, 0.0),
        (5.0, 0.0),
        (5.0, 2.0),
        (3.0, 2.0),
        (3.0, 1.0),
        (0.0, 1.0),
    ]
}

/// Deep concave spiral bay. The corridor is one unit wide, so an outward offset
/// of 0.6 has to follow the pocket instead of cutting across it.
///
/// The historical vertex list crossed itself (edges 1 and 16, among others).
/// jagua-rs 1.1.0 rejects that polygon. This outline is the same kind of bay
/// traced as a simple boundary.
fn spiral_polygon() -> Vec<(f32, f32)> {
    vec![
        (0.0, 4.0),
        (0.0, 10.0),
        (10.0, 10.0),
        (10.0, 0.0),
        (9.0, 0.0),
        (9.0, 8.0),
        (2.0, 8.0),
        (2.0, 2.0),
        (7.0, 2.0),
        (7.0, 6.0),
        (4.0, 6.0),
        (4.0, 4.0),
        (5.0, 4.0),
        (5.0, 5.0),
        (6.0, 5.0),
        (6.0, 3.0),
        (3.0, 3.0),
        (3.0, 7.0),
        (8.0, 7.0),
        (8.0, 1.0),
        (1.0, 1.0),
        (1.0, 0.0),
        (0.0, 0.0),
    ]
}

fn ext_item(id: u64, demand: u64, points: Vec<(f32, f32)>) -> ExtDemandItem {
    ExtDemandItem {
        base: ExtItem {
            id,
            orientation: ExtOrientation {
                rotation: ExtRotation::Discrete {
                    angles: vec![0.0],
                },
                reflection_axes: Vec::new(),
            },
            shape: ExtShape::SimplePolygon(ExtSPolygon(points)),
            min_quality: None,
        },
        demand,
    }
}

fn contour_clearance(inner: &SPolygon, outer: &SPolygon) -> f32 {
    let mut min_signed = f32::MAX;
    let count = inner.n_vertices();
    for index in 0..count {
        let start = inner.vertex(index);
        let end = inner.vertex((index + 1) % count);
        // Dense enough to catch a chord that cuts a corner between vertices.
        let steps = 24;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let point = Point(
                start.0 + (end.0 - start.0) * t,
                start.1 + (end.1 - start.1) * t,
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

fn polygons_overlap(a: &SPolygon, b: &SPolygon) -> bool {
    a.vertices().iter().any(|point| b.collides_with(point))
        || b.vertices().iter().any(|point| a.collides_with(point))
        || a.edge_iter()
            .any(|edge| b.edge_iter().any(|other| edge.collides_with(&other)))
}

/// Minimum distance between two disjoint contours. Zero when they intersect.
fn polygons_min_distance(a: &SPolygon, b: &SPolygon) -> f32 {
    if polygons_overlap(a, b) {
        return 0.0;
    }
    let mut min_distance = f32::MAX;
    for edge in a.edge_iter() {
        for vertex in b.vertices() {
            min_distance = min_distance.min(edge.distance_to(vertex));
        }
    }
    for edge in b.edge_iter() {
        for vertex in a.vertices() {
            min_distance = min_distance.min(edge.distance_to(vertex));
        }
    }
    min_distance
}

fn distance_to_strip(poly: &SPolygon, width: f32, height: f32) -> f32 {
    let mut min_distance = f32::MAX;
    for vertex in poly.vertices() {
        if vertex.0 < 0.0 || vertex.1 < 0.0 || vertex.0 > width || vertex.1 > height {
            return 0.0;
        }
        min_distance = min_distance
            .min(vertex.0)
            .min(vertex.1)
            .min(width - vertex.0)
            .min(height - vertex.1);
    }
    min_distance
}

fn placed_original(instance: &SPInstance, item_idx: usize, transf: &jagua_rs::geometry::DTransformation) -> SPolygon {
    let item = instance.item(item_idx);
    let external = int_to_ext_transformation(transf, &item.shape_orig().pre_transform);
    item.shape_orig()
        .shape
        .transform_clone(&external.compose())
}

fn import(ext: &ExtSPInstance, tolerance: Option<f32>) -> SPInstance {
    let config = DEFAULT_SPARROW_CONFIG;
    let importer = Importer::new(
        config.cde_config,
        tolerance,
        config.narrow_concavity_cutoff_ratio,
    );
    jagua_rs::probs::spp::io::import_instance(&importer, ext).expect("import")
}

#[test]
fn dataset_contours_are_fully_offset_by_a_margin() {
    // Same magnitude as the fork's old default separation (3 mm at the buggy DPI).
    let separation = 0.4464_f32;
    let half = separation / 2.0;
    let tolerance = (1e-3_f32).max(separation * 1e-3);
    let mut files = 0;
    for entry in std::fs::read_dir("data/input").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        files += 1;
        let mut loaded = sparrow::util::io::read_spp_input(&path)
            .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        loaded.instance.min_item_separation = separation;
        let instance = import(&loaded.instance, DEFAULT_SPARROW_CONFIG.poly_simpl_tolerance);
        for (item, _) in &instance.items {
            let original = item.shape_orig();
            let centered = original
                .shape
                .transform_clone(&original.pre_transform.compose());
            let clearance = contour_clearance(&centered, item.shape_cd().as_ref());
            assert!(
                clearance + tolerance >= half,
                "{} item {}: collision clearance {clearance:.4}, need >= {half:.4}",
                loaded.instance.name,
                item.external_id()
            );
        }
    }
    assert!(files > 0, "no instances found in data/input");
}

#[test]
fn irregular_contours_keep_min_distance_everywhere() {
    // Half of each value is large relative to a notch, which is where the
    // straight-skeleton buffer stops following the contour.
    // Demand and strip height are chosen so the initial strip is wider than the
    // inset. The gap assertions below do not depend on that width.
    let cases = [
        ("step", 4.0_f32, 8_u64, 12.0_f32, step_polygon()),
        ("spiral", 1.2_f32, 2_u64, 40.0_f32, spiral_polygon()),
    ];

    for (name, separation, demand, strip_height, points) in cases {
        let ext = ExtSPInstance {
            name: format!("concave_{name}"),
            min_item_separation: separation,
            strip_height,
            items: vec![ext_item(0, demand, points)],
        };
        // Production also simplifies. That must not pull the collision shape
        // back inside the required offset.
        let instance = import(&ext, Some(0.001));

        let half = separation / 2.0;
        let clearance_tol = (1e-3_f32).max(separation * 1e-3);
        for (item, _) in &instance.items {
            let original = item.shape_orig();
            let centered = original
                .shape
                .transform_clone(&original.pre_transform.compose());
            let clearance = contour_clearance(&centered, item.shape_cd().as_ref());
            assert!(
                clearance + clearance_tol >= half,
                "{name}: collision shape is not an outward offset of the full contour \
                 (clearance {clearance:.4}, need >= {half:.4})"
            );
        }

        let rng = Xoshiro256PlusPlus::seed_from_u64(1);
        let sample_config = SampleConfig {
            n_container_samples: 80,
            n_focussed_samples: 0,
            n_coord_descents: 1,
        };
        let builder = LBFBuilder::new(instance.clone(), rng, sample_config)
            .unwrap_or_else(|err| panic!("{name}: builder failed: {err}"))
            .construct()
            .unwrap_or_else(|err| panic!("{name}: construct failed: {err}"));
        let placed = builder
            .prob
            .layout()
            .placed_items()
            .values()
            .map(|placed| placed_original(&instance, placed.item().idx(), &placed.d_transf()))
            .collect::<Vec<_>>();
        assert_eq!(
            placed.len(),
            demand as usize,
            "{name}: expected every copy to be placed"
        );

        let width = builder.prob.strip_width();
        let height = builder.prob.strip().fixed_height;
        for (index, poly) in placed.iter().enumerate() {
            let wall = distance_to_strip(poly, width, height);
            assert!(
                wall + clearance_tol >= separation,
                "{name}: piece {index} is {wall:.4} from the strip, need >= {separation:.4}"
            );
        }
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                assert!(
                    !polygons_overlap(&placed[i], &placed[j]),
                    "{name}: placed originals {i} and {j} intersect"
                );
                let gap = polygons_min_distance(&placed[i], &placed[j]);
                assert!(
                    gap + clearance_tol >= separation,
                    "{name}: gap between originals {i} and {j} is {gap:.4}, need >= {separation:.4}"
                );
            }
        }
    }
}
