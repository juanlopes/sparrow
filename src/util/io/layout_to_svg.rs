use crate::util::io::svg_util::SvgDrawOptions;
use crate::util::io::{svg_export, svg_util};
use jagua_rs::io::parser;
use log::warn;
use std::hash::{DefaultHasher, Hash, Hasher};
use jagua_rs::collision_detection::hazards::detector::{BasicHazardDetector, HazardDetector};
use jagua_rs::collision_detection::hazards::filter::NoHazardFilter;
use jagua_rs::collision_detection::hazards::HazardEntity;
use jagua_rs::entities::general::{Instance, Layout, LayoutSnapshot};
use jagua_rs::geometry::geo_traits::Transformable;
use jagua_rs::geometry::primitives::{Circle, Edge};
use jagua_rs::geometry::{DTransformation, Transformation};
use svg::node::element::{Definitions, Group, Text, Title, Use};
use svg::Document;

pub fn s_layout_to_svg(
    s_layout: &LayoutSnapshot,
    instance: &impl Instance,
    options: SvgDrawOptions,
    title: &str,
) -> Document {
    let layout = Layout::from_snapshot(s_layout);
    layout_to_svg(&layout, instance, options, title)
}

pub fn layout_to_svg(
    layout: &Layout,
    instance: &impl Instance,
    options: SvgDrawOptions,
    title: &str,
) -> Document {
    let bin = &layout.bin;

    let vbox = bin.bbox().clone().scale(1.10);

    let theme = &options.theme;

    let stroke_width =
        f32::min(vbox.width(), vbox.height()) * 0.001 * theme.stroke_width_multiplier;

    let label = {
        //print some information on above the left top of the bin
        let label_content = format!(
            "height: {:.3} | width: {:.3} | usage: {:.3}% | {}",
            layout.bin.bbox().height(),
            layout.bin.bbox().width(),
            layout.density(instance) * 100.0,
            title,
        );
        Text::new(label_content)
            .set("x", bin.bbox().x_min)
            .set("y", bin.bbox().y_min - 0.5 * 0.025 * f32::min(bin.bbox().width(), bin.bbox().height()))
            .set("font-size", f32::min(bin.bbox().width(), bin.bbox().height()) * 0.025)
            .set("font-family", "monospace")
            .set("font-weight", "500")
    };

    //draw bin
    let bin_group = {
        let bin_group = Group::new().set("id", format!("bin_{}", bin.id));
        let bbox = bin.bbox();
        let title = Title::new(format!(
            "bin, id: {}, bbox: [x_min: {:.3}, y_min: {:.3}, x_max: {:.3}, y_max: {:.3}]",
            bin.id, bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max
        ));

        //outer
        bin_group
            .add(svg_export::data_to_path(
                svg_export::original_shape_data(&bin.original_outer, &bin.outer, options.use_internal_shapes),
                &[
                    ("fill", &*format!("{}", theme.bin_fill)),
                    ("stroke", "black"),
                    ("stroke-width", &*format!("{}", 2.0 * stroke_width)),
                ],
            ))
            .add(title)
    };

    let qz_group = {
        let mut qz_group = Group::new().set("id", "quality_zones");

        //quality zones
        for qz in bin.quality_zones.iter().rev().flatten() {
            let color = theme.qz_fill[qz.quality];
            let stroke_color = svg_util::change_brightness(color, 0.5);
            for (orig_qz_shape, intern_qz_shape) in qz.original_shapes.iter().zip(qz.shapes.iter()) {
                qz_group = qz_group.add(
                    svg_export::data_to_path(
                        svg_export::original_shape_data(orig_qz_shape, intern_qz_shape, options.use_internal_shapes),
                        &[
                            ("fill", &*format!("{}", color)),
                            ("fill-opacity", "0.50"),
                            ("stroke", &*format!("{}", stroke_color)),
                            ("stroke-width", &*format!("{}", 2.0 * stroke_width)),
                            ("stroke-opacity", &*format!("{}", theme.qz_stroke_opac)),
                            ("stroke-dasharray", &*format!("{}", 5.0 * stroke_width)),
                            ("stroke-linecap", "round"),
                            ("stroke-linejoin", "round"),
                        ],
                    )
                        .add(Title::new(format!("quality zone, q: {}", qz.quality))),
                );
            }
        }
        qz_group
    };

    //draw items
    let (items_group, surrogate_group) = {
        //define all the items and their surrogates (if enabled)
        let mut item_defs = Definitions::new();
        let mut surrogate_defs = Definitions::new();
        for (item, _) in instance.items() {
            let color = match item.base_quality {
                None => theme.item_fill.to_owned(),
                Some(q) => svg_util::blend_colors(theme.item_fill, theme.qz_fill[q]),
            };
            item_defs = item_defs.add(Group::new().set("id", format!("item_{}", item.id)).add(
                svg_export::data_to_path(
                    svg_export::original_shape_data(
                        &item.original_shape,
                        &item.shape,
                        options.use_internal_shapes,
                    ),
                    &[
                        ("fill", &*format!("{}", color)),
                        ("stroke-width", &*format!("{}", stroke_width)),
                        ("fill-rule", "nonzero"),
                        ("stroke", "black"),
                        ("fill-opacity", "0.5"),
                    ],
                ),
            ));

            if options.surrogate {
                let surr_transf = match options.use_internal_shapes {
                    true => Transformation::empty(), //surrogate is already in internal coordinates
                    false => {
                        // The original shape is drawn on the SVG, we need to inverse the pre-transform
                        let pre_transform = item.original_shape.pre_transform.compose();
                        let inv_pre_transform = pre_transform.inverse();
                        inv_pre_transform
                    }
                };
                let mut surrogate_group = Group::new().set("id", format!("surrogate_{}", item.id));
                let poi_style = [
                    ("fill", "black"),
                    ("fill-opacity", "0.1"),
                    ("stroke", "black"),
                    ("stroke-width", &*format!("{}", stroke_width)),
                    ("stroke-opacity", "0.8"),
                ];
                let ff_style = [
                    ("fill", "none"),
                    ("stroke", "black"),
                    ("stroke-width", &*format!("{}", stroke_width)),
                    ("stroke-opacity", "0.8"),
                ];
                let no_ff_style = [
                    ("fill", "none"),
                    ("stroke", "black"),
                    ("stroke-width", &*format!("{}", stroke_width)),
                    ("stroke-opacity", "0.5"),
                    ("stroke-dasharray", &*format!("{}", 5.0 * stroke_width)),
                    ("stroke-linecap", "round"),
                    ("stroke-linejoin", "round"),
                ];

                let surrogate = item.shape.surrogate();
                let poi = &surrogate.poles[0];
                let ff_poles = surrogate.ff_poles();

                for pole in surrogate.poles.iter() {
                    if pole == poi {
                        let svg_circle =
                            svg_export::circle(pole.transform_clone(&surr_transf), &poi_style);
                        surrogate_group = surrogate_group.add(svg_circle);
                    } else if ff_poles.contains(pole) {
                        let svg_circle =
                            svg_export::circle(pole.transform_clone(&surr_transf), &ff_style);
                        surrogate_group = surrogate_group.add(svg_circle);
                    } else {
                        let svg_circle =
                            svg_export::circle(pole.transform_clone(&surr_transf), &no_ff_style);
                        surrogate_group = surrogate_group.add(svg_circle);
                    }
                }
                for pier in &surrogate.piers {
                    surrogate_group = surrogate_group.add(svg_export::data_to_path(
                        svg_export::edge_data(pier.transform_clone(&surr_transf)),
                        &ff_style,
                    ));
                }
                surrogate_defs = surrogate_defs.add(surrogate_group)
            }
        }
        let mut items_group = Group::new().set("id", "items").add(item_defs);
        let mut surrogate_group = Group::new().set("id", "surrogates").add(surrogate_defs);

        for pi in layout.placed_items().values() {
            let dtransf = match options.use_internal_shapes {
                true => pi.d_transf,
                false => {
                    let item = instance.item(pi.item_id);
                    parser::internal_to_absolute_transform(
                        &pi.d_transf,
                        &item.original_shape.pre_transform,
                    )
                }
            };
            let title = Title::new(format!("item, id: {}, transf: [{}]", pi.item_id, dtransf));
            let pi_ref = Use::new()
                .set("transform", transform_to_svg(dtransf))
                .set("xlink:href", format!("#item_{}", pi.item_id))
                .add(title);

            items_group = items_group.add(pi_ref);

            if options.surrogate {
                let pi_surr_ref = Use::new()
                    .set("transform", transform_to_svg(dtransf))
                    .set("xlink:href", format!("#surrogate_{}", pi.item_id));

                surrogate_group = surrogate_group.add(pi_surr_ref);
            }
        }

        match options.surrogate {
            false => (items_group, None),
            true => (items_group, Some(surrogate_group)),
        }
    };

    //draw quadtree (if enabled)
    let qt_group = match options.quadtree {
        false => None,
        true => {
            let qt_data = svg_export::quad_tree_data(layout.cde().quadtree(), &NoHazardFilter);
            let qt_group = Group::new()
                .set("id", "quadtree")
                .add(svg_export::data_to_path(
                    qt_data.0,
                    &[
                        ("fill", "red"),
                        ("stroke-width", &*format!("{}", stroke_width * 0.25)),
                        ("fill-rule", "nonzero"),
                        ("fill-opacity", "0.6"),
                        ("stroke", "black"),
                    ],
                ))
                .add(svg_export::data_to_path(
                    qt_data.1,
                    &[
                        ("fill", "none"),
                        ("stroke-width", &*format!("{}", stroke_width * 0.25)),
                        ("fill-rule", "nonzero"),
                        ("fill-opacity", "0.3"),
                        ("stroke", "black"),
                    ],
                ))
                .add(svg_export::data_to_path(
                    qt_data.2,
                    &[
                        ("fill", "green"),
                        ("fill-opacity", "0.6"),
                        ("stroke-width", &*format!("{}", stroke_width * 0.25)),
                        ("stroke", "black"),
                    ],
                ));
            Some(qt_group)
        }
    };

    //draw hazard proximity grid (if enabled)
    let hpg_group = match options.haz_prox_grid {
        false => None,
        true => {
            let mut hpg_group = Group::new().set("id", "haz_prox_grid");
            let hpg = layout.cde().haz_prox_grid().unwrap();
            for hp_cell in hpg.grid.cells.iter().flatten() {
                let center = hp_cell.centroid;
                let prox = hp_cell.hazard_proximity(None);
                let color = if prox == 0.0 { "red" } else { "blue" };

                hpg_group = hpg_group
                    .add(svg_export::point(center, Some(color), Some(stroke_width)))
                    .add(svg_export::circle(
                        Circle::new(center, prox),
                        &[
                            ("fill", "none"),
                            ("stroke", color),
                            ("stroke-width", &*format!("{}", stroke_width / 2.0)),
                        ],
                    ));
            }
            Some(hpg_group)
        }
    };

    //highlight colliding items (if enabled)
    let collision_group = match options.highlight_collisions {
        false => None,
        true => {
            let mut collision_group = Group::new()
                .set("id", "collision_lines");
            for (pk, pi) in layout.placed_items().iter() {
                let detector = {
                    let mut detector = BasicHazardDetector::new();
                    layout.cde().collect_poly_collisions(pi.shape.as_ref(), &mut detector);
                    detector.remove(&HazardEntity::from((pk, pi)));
                    detector
                };
                for haz_entity in detector.iter() {
                    match haz_entity {
                        HazardEntity::PlacedItem { pk: colliding_pk, .. } => {
                            let haz_hash = {
                                let mut hasher = DefaultHasher::new();
                                haz_entity.hash(&mut hasher);
                                hasher.finish()
                            };
                            let pi_hash = {
                                let mut hasher = DefaultHasher::new();
                                HazardEntity::from((pk, pi)).hash(&mut hasher);
                                hasher.finish()
                            };

                            if haz_hash < pi_hash {
                                // avoid duplicate lines
                                let start = pi.shape.poi.center;
                                let end = layout.placed_items[*colliding_pk].shape.poi.center;
                                collision_group = collision_group.add(svg_export::data_to_path(
                                    svg_export::edge_data(Edge { start, end }),
                                    &[
                                        ("stroke", &*format!("{}", theme.collision_highlight_color)),
                                        ("stroke-opacity", "0.75"),
                                        ("stroke-width", &*format!("{}", stroke_width * 4.0)),
                                        ("stroke-dasharray", &*format!("{} {}", 4.0 * stroke_width, 8.0 * stroke_width)),
                                        ("stroke-linecap", "round"),
                                        ("stroke-linejoin", "round"),
                                    ],
                                ));
                            }
                        }
                        HazardEntity::BinExterior => {
                            collision_group = collision_group.add(svg_export::point(
                                pi.shape.poi.center,
                                Some(&*format!("{}", theme.collision_highlight_color)),
                                Some(3.0 * stroke_width),
                            ));
                        }
                        _ => {
                            warn!("unexpected hazard entity");
                        }
                    }
                }
            }
            Some(collision_group)
        }
    };

    let vbox_svg = (vbox.x_min, vbox.y_min, vbox.width(), vbox.height());

    let optionals = [surrogate_group, qt_group, hpg_group, collision_group]
        .into_iter()
        .flatten()
        .fold(Group::new().set("id", "optionals"), |g, opt| g.add(opt));

    Document::new()
        .set("viewBox", vbox_svg)
        .set("xmlns:xlink", "http://www.w3.org/1999/xlink")
        .add(bin_group)
        .add(items_group)
        .add(qz_group)
        .add(optionals)
        .add(label)
}
fn transform_to_svg(dt: DTransformation) -> String {
    //https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/transform
    //operations are effectively applied from right to left
    let (tx, ty) = dt.translation();
    let r = dt.rotation().to_degrees();
    format!("translate({tx} {ty}), rotate({r})")
}
