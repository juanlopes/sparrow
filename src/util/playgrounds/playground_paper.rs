use itertools::Itertools;
use jagua_rs::collision_detection::hazard_helpers::HazardDetector;
use jagua_rs::entities::bin::Bin;
use jagua_rs::entities::instances::instance::Instance;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::layout::Layout;
use jagua_rs::geometry::d_transformation::DTransformation;
use jagua_rs::geometry::primitives::aa_rectangle::AARectangle;
use jagua_rs::io::parser::Parser;
use jagua_rs::util::config::{CDEConfig, SPSurrogateConfig};
use jagua_rs::util::polygon_simplification::PolySimplConfig;
use sparrow::util::io;
use sparrow::util::io::svg_export;
use std::path::Path;
use svg::Document;

const INSTANCE_PATH: &str = "libs/jagua-rs/assets/swim.json";
const OUTPUT_FOLDER: &str = "output/playground/";

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

    let bbox = AARectangle::new(-1000.0, -500.0, 1000.0, 500.0);
    let dummy_bin = Bin::from_strip(bbox.clone(), cde_config);
    let mut dummy_layout = Layout::new(0, dummy_bin);

    let items_to_place = [
        (7, DTransformation::new(0.0, (500.0, 000.0))),
        (8, DTransformation::new(0.0, (-500.0, -50.0))),
        //(8, DTransformation::new(0.0, (400.0, -50.0))),
        //(8, DTransformation::new(0.0, (-300.0, -50.0))),
    ];

    for (item_id, transf) in items_to_place.iter() {
        dummy_layout.place_item(sp_instance.item(*item_id), transf.clone());
    }

    let doc = Document::new().set(
        "viewBox",
        (bbox.x_min, bbox.y_min, bbox.width(), bbox.height()),
    );

    let item_paths = dummy_layout
        .placed_items()
        .iter()
        .map(|(_, pi)| {
            svg_export::data_to_path(
                svg_export::simple_polygon_data(&pi.shape),
                &[
                    ("fill", "rgba(122, 122, 122, 0.5)"),
                    ("stroke-width", "15"),
                    ("fill-rule", "nonzero"),
                    ("stroke", "black")
                ],
            )
        })
        .collect_vec();

    let poles_paths = dummy_layout
        .placed_items()
        .iter()
        .flat_map(|(_, pi)| {
            pi.shape
                .surrogate()
                .poles
                .iter()
                .map(|p| svg_export::circle(p, &[
                    ("fill", "rgba(255, 255, 255, 0.5)"),
                    ("stroke", "black"),
                    ("stroke-width", "10")
                ])
                )
        })
        .collect_vec();

    let doc = item_paths.into_iter().fold(doc, |doc, path| doc.add(path));
    let doc = poles_paths.into_iter().fold(doc, |doc, path| doc.add(path));

    io::write_svg(
        &doc,
        &*Path::new(OUTPUT_FOLDER).join("paper_visualizer.svg"),
        log::Level::Info,
    );
}

