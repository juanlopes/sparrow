use gls_strip_packing::io;
use gls_strip_packing::io::svg_export;
use itertools::Itertools;
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::bin::Bin;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::item::Item;
use jagua_rs::entities::layout::Layout;
use jagua_rs::fsize;
use jagua_rs::PI;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::geo_enums::{GeoPosition, GeoRelation};
use jagua_rs::geometry::geo_traits::{Distance, Shape, Transformable};
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::geometry::primitives::circle::Circle;
use jagua_rs::geometry::primitives::simple_polygon::SimplePolygon;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use ordered_float::{Float, OrderedFloat};
use std::cmp::Ordering;
use std::path::Path;
use svg::node::element::Group;
use svg::Document;

const INSTANCE_PATH: &str = "../jagua-rs/assets/swim.json";
const ITEM_ID_TO_SAMPLE: usize = 4;

const OUTPUT_FOLDER: &str = "../output/playground/";

const RESOLUTION: usize = 400;

const ZOOM: fsize = 1.2;

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


    let diameter = 3000.0;
    let bbox = AARectangle::new(-diameter, -diameter, diameter, diameter);
    let dummy_bin = Bin::from_strip(bbox.clone(), cde_config);
    let mut dummy_layout = Layout::new(0, dummy_bin);

    let items_to_place = [
        (2, DTransformation::new(0.0, (0.0, 1000.0))),
    ];


    for (item_id, transf) in items_to_place.iter() {
        dummy_layout.place_item(sp_instance.item(*item_id), transf.clone());
    }

    println!("item_placed areas: {:?}, item_sample area: {}", dummy_layout.placed_items.values().map(|pi| pi.shape.area).collect_vec(), item_to_sample.shape.area());

    let mut overlaps = [[Overlap::None; RESOLUTION]; RESOLUTION];

    for sx in 0..RESOLUTION {
        for sy in 0..RESOLUTION {
            let x = bbox.x_min + bbox.width() / RESOLUTION as fsize * sx as fsize;
            let y = bbox.y_min + bbox.height() / RESOLUTION as fsize * sy as fsize;

            let dt = DTransformation::new(0.0, (x, y));
            let overlap = eval(&dummy_layout, item_to_sample, dt);
            overlaps[sx][sy] = overlap;
        }
    }
    let max_overlap = overlaps.iter().map(|row| row.iter().cloned())
        .flatten()
        .filter_map(|o| match o {
            Overlap::Items(o) => Some(o),
            _ => None,
        }).max_by_key(|&o| OrderedFloat(o)).unwrap();
    let min_overlap = overlaps.iter().map(|row| row.iter().cloned())
        .flatten()
        .filter_map(|o| match o {
            Overlap::Items(o) => Some(o),
            _ => None,
        }).min_by_key(|&o| OrderedFloat(o)).unwrap();

    println!("max_overlap: {}, min_overlap: {}", max_overlap, min_overlap);

    let doc = Document::new().set(
        "viewBox",
        (bbox.x_min,
         bbox.y_min,
         bbox.width(),
         bbox.height()),
    );

    let stroke_width =
        fsize::min(bbox.width(), bbox.height()) * 0.001 * 2.0;

    let item_paths = dummy_layout.placed_items()
        .iter().map(|(_, pi)| svg_export::data_to_path(
        svg_export::simple_polygon_data(&pi.shape),
        &[
            ("fill", "rgba(0, 0, 0, 0.3)"),
            ("stroke-width", &*format!("{}", stroke_width)),
            ("fill-rule", "nonzero"),
            ("stroke", "black"),
            ("opacity", "0.5"),
        ],
    ))
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

    let margin = bbox.width() / RESOLUTION as fsize * 0.01;
    for sx in 0..RESOLUTION {
        for sy in 0..RESOLUTION {
            let x = bbox.x_min + bbox.width() / RESOLUTION as fsize * sx as fsize;
            let y = bbox.y_min + bbox.height() / RESOLUTION as fsize * sy as fsize;
            let overlap = overlaps[sx][sy];
            let color = match overlap {
                Overlap::Items(o) => {
                    let gradient = 255.0 * (1.0 - o / max_overlap);
                    format!("rgb(255, {}, {})", gradient, gradient)
                },
                Overlap::None => "rgb(255, 255, 255)".to_string(),
                Overlap::Bin => "rgb(200, 200, 200)".to_string(),
            };
            let rect = svg::node::element::Rectangle::new()
                .set("x", x - margin)
                .set("y", y - margin)
                .set("width", bbox.width() / RESOLUTION as fsize + 2.0 * margin)
                .set("height", bbox.height() / RESOLUTION as fsize + 2.0 * margin)
                .set("fill", color);
            overlap_group = overlap_group.add(rect);
        }
    }

    let doc = doc.add(overlap_group);

    let doc = item_paths.into_iter().fold(doc, |doc, path| doc.add(path));
    let doc = doc.add(other_item_path);


    let doc = doc.add(svg_export::circle(
        &Circle::new((0.0, 0.0).into(), stroke_width),
        &[
            ("fill", "blue"),
            ("opacity", "1.0"),
        ]));

    io::write_svg(&doc, &*Path::new(OUTPUT_FOLDER).join("overlap_visualizer.svg"));
}

#[derive(Copy, Clone)]
enum Overlap {
    None,
    Bin,
    Items(fsize)
}

fn eval(layout: &Layout, item: &Item, dt: DTransformation) -> Overlap {
    let t_shape = item.shape.transform_clone(&dt.compose());

    if t_shape.bbox().relation_to(&layout.bin.bbox()) != GeoRelation::Enclosed {
        Overlap::Bin
    } else {
        let mut colliding_buffer = Vec::new();
        layout.cde().collect_poly_collisions(&t_shape, &[], &mut colliding_buffer);
        if colliding_buffer.is_empty() {
            Overlap::None
        } else if colliding_buffer.contains(&HazardEntity::BinExterior) {
            Overlap::Bin
        } else {
            let o = colliding_buffer.iter()
                .map(|haz| layout.hazard_to_p_item_key(haz).unwrap())
                .map(|pik| layout.placed_items()[pik].shape.as_ref())
                .map(|other_shape| poly_overlap_proxy(&t_shape, other_shape))
                .sum();
            Overlap::Items(o)
        }
    }
}

pub fn poly_overlap_proxy(s1: &SimplePolygon, s2: &SimplePolygon) -> fsize {
    const MARGIN_FRAC: fsize = 0.01;
    let margin = (s1.diameter + s2.diameter) / 2.0 * MARGIN_FRAC;

    dbg!(margin);

    let deficit = poles_overlap_proxy(
        s1.surrogate().poles.iter(),
        s2.surrogate().poles.iter(),
        margin,
    );

    let s1_penalty = (s1.surrogate().convex_hull_area); //+ //0.1 * (s1.diameter / 4.0).powi(2));
    let s2_penalty = (s2.surrogate().convex_hull_area); // + 0.1 * (s2.diameter / 4.0).powi(2));

    let penalty = 0.99 * fsize::min(s1_penalty, s2_penalty) + 0.01 * fsize::max(s1_penalty, s2_penalty);

    (deficit + 0.000 * penalty).sqrt() * penalty.sqrt()
}

pub fn poles_overlap_proxy<'a, C>(poles_1: C, poles_2: C, margin: fsize) -> fsize
where
    C: Iterator<Item=&'a Circle> + Clone,
{
    let mut deficit = 0.0;
    for p1 in poles_1 {
        for p2 in poles_2.clone() {
            let d = match p1.separation_distance(p2) {
                (GeoPosition::Interior, d) => {
                    d + margin
                },
                (GeoPosition::Exterior, d) => margin / (d / margin + 1.0),
            };
            deficit += d * fsize::min(p1.radius, p2.radius);
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