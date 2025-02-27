use itertools::Itertools;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::bin::Bin;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::fsize;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_enums::GeoRelation;
use jagua_rs::geometry::geo_traits::{Distance, Shape, Transformable};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::circle::Circle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use ordered_float::{Float, OrderedFloat};
use plotly::common::{ColorScale, ColorScaleElement};
use plotly::layout::{AspectMode, AspectRatio, LayoutScene};
use plotly::{Plot, Surface};
use std::path::Path;
use svg::Document;
use svg::node::element::Group;
use gls_strip_packing::util::io;
use gls_strip_packing::util::io::svg_export;

const INSTANCE_PATH: &str = "libs/jagua-rs/assets/swim.json";
const ITEM_ID_TO_SAMPLE: usize = 7;

const OUTPUT_FOLDER: &str = "output/playground/";

const RESOLUTION: usize = 1000;

const ZOOM: fsize = 1.2;

const EPSILON_DIAM_FRAC: fsize = 0.005;

pub fn main() {
    io::init_logger(log::LevelFilter::Info);

    let cde_config = CDEConfig {
        quadtree_depth: 4,
        hpg_n_cells: 2000,
        item_surrogate_config: SPSurrogateConfig {
            pole_coverage_goal: 0.95,
            max_poles: 20,
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

    let diameter = 5000.0;
    let bbox = AARectangle::new(-diameter, -diameter, diameter, diameter);
    let dummy_bin = Bin::from_strip(bbox.clone(), cde_config);
    let mut dummy_layout = Layout::new(0, dummy_bin);

    let items_to_place = [
        (9, DTransformation::new(0.0, (1000.0, -1500.0))),
        (4, DTransformation::new(0.0, (1000.0, 1500.0))),
        (8, DTransformation::new(0.0, (-1000.0, 250.0))),
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
            let x = bbox.x_min + bbox.width() / RESOLUTION as fsize * sx as fsize;
            let y = bbox.y_min + bbox.height() / RESOLUTION as fsize * sy as fsize;

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
    );

    let stroke_width = fsize::min(bbox.width(), bbox.height()) * 0.001 * 2.0;

    let item_paths = dummy_layout
        .placed_items()
        .iter()
        .map(|(_, pi)| {
            svg_export::data_to_path(
                svg_export::simple_polygon_data(&pi.shape),
                &[
                    ("fill", "rgba(0, 0, 0, 0.0)"),
                    ("stroke-width", &*format!("{}", stroke_width)),
                    ("fill-rule", "nonzero"),
                    ("stroke", "black"),
                    ("opacity", "0.5"),
                ],
            )
        })
        .collect_vec();

    let other_item_path = svg_export::data_to_path(
        svg_export::simple_polygon_data(&item_to_sample.shape),
        &[
            ("fill", "none"),
            ("stroke-width", &*format!("{}", stroke_width)),
            ("fill-rule", "nonzero"),
            ("stroke", "black"),
            ("opacity", "0.2"),
        ],
    );

    //overlay the overlaps

    let mut overlap_group = Group::new();

    let margin = bbox.width() / RESOLUTION as fsize * 0.1;
    for sx in 0..RESOLUTION {
        for sy in 0..RESOLUTION {
            let x = bbox.x_min + bbox.width() / RESOLUTION as fsize * sx as fsize;
            let y = bbox.y_min + bbox.height() / RESOLUTION as fsize * sy as fsize;
            let overlap = overlaps[sx * RESOLUTION + sy];
            let color = match overlap {
                Overlap::Items(o) => {
                    let gradient = 255.0 * (1.0 - o / max_overlap);
                    Some(format!("rgb(255, {}, {})", gradient, gradient))
                }
                Overlap::None => None,
                Overlap::Bin => None,
                Overlap::BoundaryNone => Some(format!("rgb(220, 255, 220)")),
            };
            if let Some(color) = color {
                let rect = svg::node::element::Rectangle::new()
                    .set("x", x - margin)
                    .set("y", y - margin)
                    .set("width", bbox.width() / RESOLUTION as fsize + 2.0 * margin)
                    .set("height", bbox.height() / RESOLUTION as fsize + 2.0 * margin)
                    .set("fill", color);
                overlap_group = overlap_group.add(rect);
            }
        }
    }

    let doc = doc.add(overlap_group);

    let doc = item_paths.into_iter().fold(doc, |doc, path| doc.add(path));
    let doc = doc.add(other_item_path);

    let doc = doc.add(svg_export::circle(
        &Circle::new((0.0, 0.0).into(), stroke_width),
        &[("fill", "blue"), ("opacity", "1.0")],
    ));

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
                Overlap::None => -10.0,
                Overlap::Bin => -10.0,
                Overlap::BoundaryNone => -10.0,
            })
            .collect::<Vec<f64>>()
            .chunks(RESOLUTION)
            .map(|chunk| chunk.to_vec())
            .collect();

        let x = (0..RESOLUTION)
            .map(|i| bbox.x_min + bbox.width() / RESOLUTION as fsize * i as fsize)
            .collect::<Vec<fsize>>();
        let y = (0..RESOLUTION)
            .map(|i| bbox.y_min + bbox.height() / RESOLUTION as fsize * i as fsize)
            .collect::<Vec<fsize>>();

        let color_scale = ColorScale::Vector(vec![
            ColorScaleElement(0.0, "#ffffff".into()), // White at 0.0
            ColorScaleElement(1.0, "#ff0000".into()), // Red at max value
        ]);

        // Create a surface plot
        let surface = Surface::new(z)
            .x(x.clone())
            .y(y.clone())
            .color_scale(color_scale);

        let green_floor_surface = {
            let color_scale = ColorScale::Vector(vec![
                ColorScaleElement(0.0, "#cdffcd".into()), // White at 0.0
                ColorScaleElement(1.0, "#cdffcd".into()), // Red at max value
            ]);
            let z = vec![vec![-1.0; RESOLUTION]; RESOLUTION];
            Surface::new(z)
                .x(x.clone())
                .y(y.clone())
                .color_scale(color_scale)
        };

        let mut plot = Plot::new();
        plot.add_trace(surface);
        plot.add_trace(green_floor_surface);

        let aspect = AspectRatio::new().x(1.0).y(1.0).z(0.3);
        let scene = LayoutScene::new()
            .aspect_mode(AspectMode::Manual)
            .aspect_ratio(aspect);

        let layout = plotly::Layout::new()
            .width(1920)
            .height(1080)
            .title("3D visualization of overlap approximation")
            .z_axis(plotly::layout::Axis::new().range(vec![0.0, max_overlap]))
            .scene(scene);
        plot.set_layout(layout);

        // Export the plot to an HTML file

        plot.write_html(&*Path::new(OUTPUT_FOLDER).join("overlap_visualizer_3d.html"));

        println!("Plot exported to plot.html");
    }
}

#[derive(Copy, Clone)]
enum Overlap {
    None,
    Bin,
    Items(fsize),
    BoundaryNone,
}

fn eval(layout: &Layout, item: &Item, dt: DTransformation) -> Overlap {
    let t_shape = item.shape.transform_clone(&dt.compose());

    if t_shape.bbox().relation_to(&layout.bin.bbox()) != GeoRelation::Enclosed {
        Overlap::Bin
    } else {
        let collisions = layout.cde().collect_poly_collisions(&t_shape, &[]);
        if collisions.is_empty() {
            Overlap::None
        } else if collisions.contains(&HazardEntity::BinExterior) {
            Overlap::Bin
        } else {
            let o = collisions
                .iter()
                .map(|haz| layout.hazard_to_p_item_key(haz).unwrap())
                .map(|pik| layout.placed_items()[pik].shape.as_ref())
                .map(|other_shape| poly_overlap_proxy(&t_shape, other_shape))
                .sum();
            Overlap::Items(o)
        }
    }
}

pub fn poly_overlap_proxy(s1: &SimplePolygon, s2: &SimplePolygon) -> fsize {
    let epsilon = fsize::max(s1.diameter, s2.diameter) * EPSILON_DIAM_FRAC;

    let deficit = poles_overlap_proxy(
        s1.surrogate().poles.iter(),
        s2.surrogate().poles.iter(),
        epsilon,
    );

    let s1_penalty = s1.surrogate().convex_hull_area; //+ //0.1 * (s1.diameter / 4.0).powi(2));
    let s2_penalty = s2.surrogate().convex_hull_area; // + 0.1 * (s2.diameter / 4.0).powi(2));

    let penalty =
        1.00 * fsize::min(s1_penalty, s2_penalty) + 0.00 * fsize::max(s1_penalty, s2_penalty);

    (deficit * penalty).sqrt()
}

pub fn poles_overlap_proxy<'a, C>(poles_1: C, poles_2: C, epsilon: fsize) -> fsize
where
    C: Iterator<Item = &'a Circle> + Clone,
{
    let mut total_deficit = 0.0;
    for p1 in poles_1 {
        for p2 in poles_2.clone() {
            let d = (p1.radius + p2.radius) - p1.center.distance(p2.center);

            let dd = match d >= epsilon {
                true => d,
                false => epsilon.powi(2) / (-d + 2.0 * epsilon),
            };

            total_deficit += dd * fsize::min(p1.radius, p2.radius);
        }
    }
    total_deficit
}

fn distance_between_bboxes(big_bbox: &AARectangle, small_bbox: &AARectangle) -> fsize {
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
