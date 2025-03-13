use itertools::Itertools;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::collision_detection::hazard_helpers::HazardDetector;
use jagua_rs::entities::bin::Bin;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_enums::GeoRelation;
use jagua_rs::geometry::geo_traits::{Distance, Shape, Transformable};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::circle::Circle;
use jagua_rs::geometry::primitives::point::Point;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use ordered_float::OrderedFloat;
use plotly::common::{ColorScale, ColorScaleElement};
use plotly::layout::{AspectMode, AspectRatio, LayoutScene};
use plotly::surface::{PlaneContours, SurfaceContours};
use plotly::{Plot, Surface};
use sparrow::util::io;
use sparrow::util::io::svg_export;
use std::path::Path;
use svg::node::element::Group;
use svg::Document;

const INSTANCE_PATH: &str = "libs/jagua-rs/assets/swim.json";
const ITEM_ID_TO_SAMPLE: usize = 8;

const OUTPUT_FOLDER: &str = "output/playground/";

const RESOLUTION: usize = 500;
//const EPSILON_DIAM_FRAC: f32 = 0.0000001;
const EPSILON_DIAM_FRAC: f32 = 0.01;


pub fn main() {
    io::init_logger(log::LevelFilter::Info);

    let cde_config = CDEConfig {
        quadtree_depth: 3,
        hpg_n_cells: 0,
        item_surrogate_config: SPSurrogateConfig {
            pole_coverage_goal: 0.8,
            max_poles: 6,
            n_ff_poles: 2,
            n_ff_piers: 0,
        },
    };

    let json_instance = io::read_json_instance(Path::new(INSTANCE_PATH));
    let poly_simpl_config = PolySimplConfig::Disabled;

    let parser = Parser::new(poly_simpl_config, cde_config, true);
    let instance = parser.parse(&json_instance);
    let sp_instance = match instance.clone() {
        Instance::SP(spi) => spi,
        _ => panic!("Only SP instances are supported"),
    };

    let item_to_sample = sp_instance.item(ITEM_ID_TO_SAMPLE);

    let bbox = AARectangle::new(-1200.0, -1200.0, 1200.0, 1200.0);
    let dummy_bin = Bin::from_strip(bbox.clone(), cde_config);
    let mut dummy_layout = Layout::new(0, dummy_bin);

    let items_to_place = [
        (7, DTransformation::new(0.0, (0.0, 0.0)))
    ];

    for (item_id, transf) in items_to_place.iter() {
        dummy_layout.place_item(sp_instance.item(*item_id), transf.clone());
    }

    println!(
        "item_placed areas: {:?}, item_sample area: {}",
        dummy_layout
            .placed_items
            .values()
            .map(|pi| pi.shape.area)
            .collect_vec(),
        item_to_sample.shape.area()
    );

    let mut overlaps = vec![Overlap::None; RESOLUTION * RESOLUTION];

    for sx in 0..RESOLUTION {
        for sy in 0..RESOLUTION {
            let x = bbox.x_min + bbox.width() / RESOLUTION as f32 * sx as f32;
            let y = bbox.y_min + bbox.height() / RESOLUTION as f32 * sy as f32;

            let dt = DTransformation::new(0.0, (x, y));
            let overlap = eval(&dummy_layout, item_to_sample, dt);
            overlaps[sx * RESOLUTION + sy] = overlap;
        }
    }
    let max_overlap = overlaps
        .iter()
        .filter_map(|o| match o {
            Overlap::Items(o) => Some(*o),
            _ => None,
        })
        .max_by_key(|&o| OrderedFloat(o))
        .unwrap();
    let min_overlap = overlaps
        .iter()
        .filter_map(|o| match o {
            Overlap::Items(o) => Some(*o),
            _ => None,
        })
        .min_by_key(|&o| OrderedFloat(o))
        .unwrap();

    for sx in 1..RESOLUTION - 1 {
        for sy in 1..RESOLUTION - 1 {
            if let Overlap::None = overlaps[sx * RESOLUTION + sy] {
                //if any neighbor is an item overlap, mark as boundary none
                let neighbors = [
                    (sx - 1, sy),
                    (sx + 1, sy),
                    (sx, sy - 1),
                    (sx, sy + 1),
                    (sx - 1, sy - 1),
                    (sx + 1, sy + 1),
                    (sx - 1, sy + 1),
                    (sx + 1, sy - 1),
                ];

                if neighbors
                    .iter()
                    .any(|(nx, ny)| matches!(overlaps[*nx * RESOLUTION + *ny], Overlap::Items(_)))
                {
                    overlaps[sx * RESOLUTION + sy] = Overlap::BoundaryNone;
                }
            }
        }
    }

    println!("max_overlap: {}, min_overlap: {}", max_overlap, min_overlap);

    let doc = Document::new().set(
        "viewBox",
        (bbox.x_min, bbox.y_min, bbox.width(), bbox.height()),
    ).add(svg::node::element::Rectangle::new()
        .set("x", bbox.x_min)
        .set("y", bbox.y_min)
        .set("width", bbox.width())
        .set("height", bbox.height())
        .set("fill", "#cdffcd"));


    let item_paths = dummy_layout
        .placed_items()
        .iter()
        .map(|(_, pi)| {
            svg_export::data_to_path(
                svg_export::simple_polygon_data(&pi.shape),
                &[
                    ("fill", "none"),
                    ("stroke-width", "15"),
                    ("fill-rule", "nonzero"),
                    ("stroke", "black"),
                    ("opacity", "1.0"),
                ],
            )
        })
        .collect_vec();

    let sample_group = {
        let idx_to_point = |i: usize| {
            let sx = i / RESOLUTION;
            let sy = i % RESOLUTION;
            let x = bbox.x_min + bbox.width() / RESOLUTION as f32 * sx as f32;
            let y = bbox.y_min + bbox.height() / RESOLUTION as f32 * sy as f32;
            Point(x, y)
        };

        let filtered_overlaps = overlaps.iter().enumerate()
            .filter(|(_, o)| o == &&Overlap::BoundaryNone)
            .map(|(idx, _)| idx_to_point(idx));

        let x_min = filtered_overlaps.clone()
            .min_by_key(|p| OrderedFloat(p.0))
            .unwrap();

        let x_max = filtered_overlaps.clone()
            .max_by_key(|p| OrderedFloat(p.0))
            .unwrap();

        let y_min = filtered_overlaps.clone()
            .min_by_key(|p| OrderedFloat(p.1))
            .unwrap();

        let y_max = filtered_overlaps.clone()
            .max_by_key(|p| OrderedFloat(p.1))
            .unwrap();

        let mut sample_group = [x_min, x_max, y_min, y_max].map(|p| {
            let t_shape = item_to_sample.shape.transform_clone(&DTransformation::new(0.0, p.into()).compose());
            svg_export::data_to_path(
                svg_export::simple_polygon_data(&t_shape),
                &[
                    ("fill", "none"),
                    ("stroke-width", "15"),
                    ("fill-rule", "nonzero"),
                    ("stroke", "black"),
                    ("opacity", "0.2"),
                ])
        }).into_iter().fold(Group::new(), |group, path| group.add(path));

        sample_group = [x_min, x_max, y_min, y_max].map(|p| {
            svg_export::circle(
                &Circle::new(p, 15.0),
                &[("fill", "black"), ("opacity", "0.5")],
            )
        }).into_iter().fold(sample_group, |group, path| group.add(path));

        sample_group
    };

    //overlay the overlaps

    let mut overlap_group = Group::new();

    let margin = bbox.width() / RESOLUTION as f32 * 0.1;
    for sx in 0..RESOLUTION {
        for sy in 0..RESOLUTION {
            let x = bbox.x_min + bbox.width() / RESOLUTION as f32 * sx as f32;
            let y = bbox.y_min + bbox.height() / RESOLUTION as f32 * sy as f32;
            let overlap = overlaps[sx * RESOLUTION + sy];
            overlap_group = match overlap {
                Overlap::Items(o) => {
                    let gradient = 255.0 * (1.0 - o / max_overlap);
                    let rect = svg::node::element::Rectangle::new()
                        .set("x", x - margin)
                        .set("y", y - margin)
                        .set("width", bbox.width() / RESOLUTION as f32 + 2.0 * margin)
                        .set("height", bbox.height() / RESOLUTION as f32 + 2.0 * margin)
                        .set("fill", format!("rgb(255, {}, {})", gradient, gradient));
                    overlap_group.add(rect)
                }
                _ => overlap_group,
            };
        }
    }

    // for sx in 0..RESOLUTION {
    //     for sy in 0..RESOLUTION {
    //         let x = bbox.x_min + bbox.width() / RESOLUTION as f32 * sx as f32;
    //         let y = bbox.y_min + bbox.height() / RESOLUTION as f32 * sy as f32;
    //         let overlap = overlaps[sx * RESOLUTION + sy];
    //         overlap_group = match overlap {
    //             Overlap::BoundaryNone => {
    //                 let circle = svg::node::element::Circle::new()
    //                     .set("cx", x)
    //                     .set("cy", y)
    //                     .set("r", 2.0 * bbox.width() / RESOLUTION as f32)
    //                     .set("fill", "rgb(100, 255, 100)")
    //                     .set("stroke", "none");
    //                 overlap_group.add(circle)
    //             }
    //             _ => overlap_group,
    //         };
    //     }
    // }

    let doc = doc.add(overlap_group);

    let doc = item_paths.into_iter().fold(doc, |doc, path| doc.add(path));
    let doc = doc.add(sample_group);

    io::write_svg(
        &doc,
        &*Path::new(OUTPUT_FOLDER).join("overlap_visualizer.svg"),
        log::Level::Info,
    );

    {
        // Flatten the matrix into a Vec of Vecs for Plotly
        let z: Vec<Vec<f64>> = overlaps
            .iter()
            .map(|o| match o {
                Overlap::Items(o) => *o as f64,
                Overlap::None => f64::NAN,
                Overlap::Bin => f64::NAN,
                Overlap::BoundaryNone => 0.0,
            })
            .collect::<Vec<f64>>()
            .chunks(RESOLUTION)
            .map(|chunk| chunk.to_vec())
            .collect();

        let x = (0..RESOLUTION)
            .map(|i| bbox.x_min + bbox.width() / RESOLUTION as f32 * i as f32)
            .collect::<Vec<f32>>();
        let y = (0..RESOLUTION)
            .map(|i| bbox.y_min + bbox.height() / RESOLUTION as f32 * i as f32)
            .collect::<Vec<f32>>();

        let color_scale = ColorScale::Vector(vec![
            ColorScaleElement(0.0, "#ffffff".into()), // White at 0.0
            ColorScaleElement(1.0, "#ff0000".into()), // Red at max value
        ]);

        let green_floor_surface = {
            let color_scale = ColorScale::Vector(vec![
                ColorScaleElement(0.0, "#cdffcd".into()),
                ColorScaleElement(1.0, "#cdffcd".into()),
            ]);
            //clone the other surface and set all NANs to 0.0 and all normal values to NAN
            let z = z
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|&v| if v.is_nan() || v == 0.0 { 0.0 } else { f64::NAN })
                        .collect::<Vec<f64>>()
                })
                .collect::<Vec<Vec<f64>>>();
            Surface::new(z)
                .x(x.clone())
                .y(y.clone())
                .color_scale(color_scale)
                .show_scale(false)
        };

        // Create a surface plot
        let surface = Surface::new(z)
            .x(x.clone())
            .y(y.clone())
            .color_scale(color_scale)
            .show_scale(false)
            .lighting(plotly::surface::Lighting::new()
                .ambient(0.8)
                .diffuse(0.8)
                .specular(0.05)
                .roughness(0.5)
            );

        let mut plot = Plot::new();
        plot.add_trace(surface);
        plot.add_trace(green_floor_surface);

        let aspect = AspectRatio::new().x(1.0).y(1.0).z(0.5);
        let scene = LayoutScene::new()
            .aspect_mode(AspectMode::Manual)
            .x_axis(plotly::layout::Axis::new().show_tick_labels(false).title("").range(vec![-1000.0, 1000.0]))
            .y_axis(plotly::layout::Axis::new().show_tick_labels(false).title("").range(vec![-1000.0, 1000.0]))
            .z_axis(plotly::layout::Axis::new().show_tick_labels(false).title("").range(vec![0.0, max_overlap]))
            .aspect_ratio(aspect);

        let layout = plotly::Layout::new()
            .width(2160)
            .height(2160)
            .scene(scene);
        plot.set_layout(layout);

        // Export the plot to an HTML file
        plot.write_html(&*Path::new(OUTPUT_FOLDER).join("overlap_visualizer_3d.html"));

        println!("Plot exported to plot.html");
    }
}

#[derive(Copy, Clone, PartialEq)]
enum Overlap {
    None,
    Bin,
    Items(f32),
    BoundaryNone,
}

fn eval(layout: &Layout, item: &Item, dt: DTransformation) -> Overlap {
    let t_shape = item.shape.transform_clone(&dt.compose());

    if t_shape.bbox().relation_to(&layout.bin.bbox()) != GeoRelation::Enclosed {
        Overlap::Bin
    } else {
        let collisions = layout.cde().collect_poly_collisions(&t_shape, &[]);
        if collisions.len() == 0 {
            Overlap::None
        } else if collisions.contains(&HazardEntity::BinExterior) {
            Overlap::Bin
        } else {
            let o = collisions.iter()
                .filter_map(|haz| match haz {
                    HazardEntity::PlacedItem { pk, .. } => Some(*pk),
                    _ => None,
                })
                .map(|pik| layout.placed_items()[pik].shape.as_ref())
                .map(|other_shape| poly_overlap_proxy(&t_shape, other_shape))
                .sum();
            Overlap::Items(o)
        }
    }
}

pub fn poly_overlap_proxy(s1: &SimplePolygon, s2: &SimplePolygon) -> f32 {
    let epsilon = f32::max(s1.diameter, s2.diameter) * EPSILON_DIAM_FRAC;

    let deficit = poles_overlap_proxy(
        s1.surrogate().poles.iter(),
        s2.surrogate().poles.iter(),
        epsilon,
    );

    let s1_penalty = s1.surrogate().convex_hull_area; //+ //0.1 * (s1.diameter / 4.0).powi(2));
    let s2_penalty = s2.surrogate().convex_hull_area; // + 0.1 * (s2.diameter / 4.0).powi(2));

    let penalty = f32::min(s1_penalty, s2_penalty);

    (deficit * penalty).sqrt()
}

pub fn poles_overlap_proxy<'a, C>(poles_1: C, poles_2: C, epsilon: f32) -> f32
where
    C: Iterator<Item=&'a Circle> + Clone,
{
    let mut total_deficit = 0.0;
    for p1 in poles_1 {
        for p2 in poles_2.clone() {
            let d = (p1.radius + p2.radius) - p1.center.distance(&p2.center);

            let dd = match d >= epsilon {
                true => d,
                false => epsilon.powi(2) / (-d + 2.0 * epsilon),
            };

            total_deficit += dd * f32::min(p1.radius, p2.radius);
        }
    }
    total_deficit
}

fn distance_between_bboxes(big_bbox: &AARectangle, small_bbox: &AARectangle) -> f32 {
    let min_d = [
        big_bbox.x_max - small_bbox.x_max,
        small_bbox.x_min - big_bbox.x_min,
        big_bbox.y_max - small_bbox.y_max,
        small_bbox.y_min - big_bbox.y_min,
    ]
        .iter()
        .min_by_key(|d| OrderedFloat(**d))
        .copied()
        .unwrap();

    assert!(min_d >= -1.0);

    min_d
}
