use crate::util::io::svg_util::SvgDrawOptions;
use crate::util::io::{svg_export, svg_util};
use jagua_rs::collision_detection::hazard::HazardEntity;
use jagua_rs::entities::instances::instance_generic::InstanceGeneric;
use jagua_rs::entities::layout::Layout;
use jagua_rs::entities::layout::LayoutSnapshot;
use jagua_rs::fsize;
use jagua_rs::geometry::primitives::circle::Circle;
use jagua_rs::geometry::primitives::edge::Edge;
use jagua_rs::geometry::transformation::Transformation;
use jagua_rs::io::parser;
use log::warn;
use std::hash::{DefaultHasher, Hash, Hasher};
use svg::Document;
use svg::node::element::{Definitions, Group, Title, Use, Text};

pub fn s_layout_to_svg(
    s_layout: &LayoutSnapshot,
    instance: &impl InstanceGeneric,
    options: SvgDrawOptions,
) -> Document {
    let layout = Layout::from_snapshot(s_layout);
    layout_to_svg(&layout, instance, options)
}

pub fn layout_to_svg(
    layout: &Layout,
    instance: &impl InstanceGeneric,
    options: SvgDrawOptions,
) -> Document {
    let internal_bin = &layout.bin;
    let inv_bin_transf = internal_bin.pretransform.clone().inverse();
    let bin = parser::pretransform_bin(internal_bin, &inv_bin_transf);

    let vbox = bin.bbox().clone().scale(1.10);

    let theme = &options.theme;

    let stroke_width =
        fsize::min(vbox.width(), vbox.height()) * 0.001 * theme.stroke_width_multiplier;

    let label = {
        //print some information on above the left top of the bin
        let label_content = format!(
            "height: {:.3} | width: {:.3} | usage: {:.3}%",
            layout.bin.bbox().height(),
            layout.bin.bbox().width(),
            layout.usage() * 100.0
        );
        Text::new(label_content)
            .set("x", bin.bbox().x_min)
            .set("y", bin.bbox().y_min - 0.5 * 0.025 * fsize::min(bin.bbox().width(), bin.bbox().height()))
            .set("font-size", fsize::min(bin.bbox().width(), bin.bbox().height()) * 0.025)
            .set("font-family", "monospace")
            .set("font-weight", "500")
    };

    //draw bin
    let bin_group = {
        let mut bin_group = Group::new().set("id", format!("bin_{}", bin.id));
        let bbox = bin.bbox();
        let title = Title::new(format!(
            "bin, id: {}, bbox: [x_min: {:.3}, y_min: {:.3}, x_max: {:.3}, y_max: {:.3}]",
            bin.id, bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max
        ));

        //outer
        bin_group = bin_group
            .add(svg_export::data_to_path(
                svg_export::simple_polygon_data(&bin.outer),
                &[
                    ("fill", &*format!("{}", theme.bin_fill)),
                    ("stroke", "black"),
                    ("stroke-width", &*format!("{}", 2.0 * stroke_width)),
                ],
            ))
            .add(title);

        //holes
        for (hole_idx, hole) in bin.holes.iter().enumerate() {
            bin_group = bin_group.add(
                svg_export::data_to_path(
                    svg_export::simple_polygon_data(hole),
                    &[
                        ("fill", &*format!("{}", theme.hole_fill)),
                        ("stroke", "black"),
                        ("stroke-width", &*format!("{}", 1.0 * stroke_width)),
                    ],
                )
                .add(Title::new(format!("hole #{}", hole_idx))),
            );
        }
        bin_group
    };

    let qz_group = {
        let mut qz_group = Group::new().set("id", "quality_zones");

        //quality zones
        for qz in bin.quality_zones.iter().rev().flatten() {
            let color = theme.qz_fill[qz.quality];
            let stroke_color = svg_util::change_brightness(color, 0.5);
            for qz_shape in qz.zones.iter() {
                qz_group = qz_group.add(
                    svg_export::data_to_path(
                        svg_export::simple_polygon_data(qz_shape),
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
        for (internal_item, _) in instance.items() {
            let item = parser::pretransform_item(
                internal_item,
                &internal_item.pretransform.clone().inverse(),
            );
            let shape = item.shape.as_ref();
            let color = match item.base_quality {
                None => theme.item_fill.to_owned(),
                Some(q) => svg_util::blend_colors(theme.item_fill, theme.qz_fill[q]),
            };
            item_defs = item_defs.add(Group::new().set("id", format!("item_{}", item.id)).add(
                svg_export::data_to_path(
                    svg_export::simple_polygon_data(shape),
                    &[
                        ("fill", &*format!("{}", color)),
                        ("stroke-width", &*format!("{}", stroke_width)),
                        ("fill-rule", "nonzero"),
                        ("stroke", "black"),
                        ("opacity", "0.9"),
                    ],
                ),
            ));

            if options.surrogate {
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
                        surrogate_group = surrogate_group.add(svg_export::circle(pole, &poi_style));
                    }
                    if ff_poles.contains(pole) {
                        surrogate_group = surrogate_group.add(svg_export::circle(pole, &ff_style));
                    } else {
                        surrogate_group =
                            surrogate_group.add(svg_export::circle(pole, &no_ff_style));
                    }
                }
                for pier in &surrogate.piers {
                    surrogate_group = surrogate_group.add(svg_export::data_to_path(
                        svg_export::edge_data(pier),
                        &ff_style,
                    ));
                }
                surrogate_defs = surrogate_defs.add(surrogate_group)
            }
        }
        let mut items_group = Group::new().set("id", "items").add(item_defs);
        let mut surrogate_group = Group::new().set("id", "surrogates").add(surrogate_defs);

        for pi in layout.placed_items().values() {
            let abs_transf = parser::internal_to_absolute_transform(
                &pi.d_transf,
                &instance.item(pi.item_id).pretransform,
                &internal_bin.pretransform,
            );
            let title = Title::new(format!(
                "item, id: {}, transf: [{}]",
                pi.item_id,
                abs_transf.decompose()
            ));
            let pi_ref = Use::new()
                .set("transform", transform_to_svg(&abs_transf))
                .set("xlink:href", format!("#item_{}", pi.item_id))
                .add(title);

            items_group = items_group.add(pi_ref);

            if options.surrogate {
                let pi_surr_ref = Use::new()
                    .set("transform", transform_to_svg(&abs_transf))
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
            let qt_data = svg_export::quad_tree_data(layout.cde().quadtree(), &[]);
            let qt_group = Group::new()
                .set("id", "quadtree")
                .set("transform", transform_to_svg(&inv_bin_transf))
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
            let mut hpg_group = Group::new()
                .set("id", "haz_prox_grid")
                .set("transform", transform_to_svg(&inv_bin_transf));
            let hpg = layout.cde().haz_prox_grid().unwrap();
            for hp_cell in hpg.grid.cells.iter().flatten() {
                let center = hp_cell.centroid;
                let prox = hp_cell.hazard_proximity(None);
                let color = if prox == 0.0 { "red" } else { "blue" };

                hpg_group = hpg_group
                    .add(svg_export::point(center, Some(color), Some(stroke_width)))
                    .add(svg_export::circle(
                        &Circle::new(center, prox),
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

    //highlight overlapping items (if enabled)
    let overlap_group = match options.highlight_overlap {
        false => None,
        true => {
            let mut overlap_group = Group::new()
                .set("id", "overlap_lines")
                .set("transform", transform_to_svg(&inv_bin_transf));
            for (_, pi) in layout.placed_items().iter() {
                let collides_with = layout
                    .cde()
                    .collect_poly_collisions(pi.shape.as_ref(), &[pi.into()]);
                for haz_entity in collides_with {
                    match haz_entity {
                        HazardEntity::PlacedItem { .. } => {
                            let haz_hash = {
                                let mut hasher = DefaultHasher::new();
                                haz_entity.hash(&mut hasher);
                                hasher.finish()
                            };
                            let pi_hash = {
                                let mut hasher = DefaultHasher::new();
                                HazardEntity::from(pi).hash(&mut hasher);
                                hasher.finish()
                            };

                            if haz_hash < pi_hash {
                                // avoid duplicate lines
                                let start = pi.shape.poi.center;
                                let colliding_pkey = layout
                                    .hazard_to_p_item_key(&haz_entity)
                                    .expect("collding placed item not found");
                                let end = layout.placed_items[colliding_pkey].shape.poi.center;
                                overlap_group = overlap_group.add(svg_export::data_to_path(
                                    svg_export::edge_data(&Edge { start, end }),
                                    &[
                                        ("stroke", &*format!("{}", theme.overlap_highlight_color)),
                                        ("stroke-width", &*format!("{}", stroke_width * 4.0)),
                                        ("stroke-dasharray", &*format!("{} {}", 4.0 * stroke_width, 8.0 * stroke_width)),
                                        ("stroke-linecap", "round"),
                                        ("stroke-linejoin", "round"),
                                    ],
                                ));
                            }
                        }
                        HazardEntity::BinExterior => {
                            overlap_group = overlap_group.add(svg_export::point(
                                pi.shape.poi.center,
                                Some(&*format!("{}", theme.overlap_highlight_color)),
                                Some(3.0 * stroke_width),
                            ));
                        }
                        _ => {
                            warn!("unexpected hazard entity");
                        }
                    }
                }
            }
            Some(overlap_group)
        }
    };

    let vbox_svg = (vbox.x_min, vbox.y_min, vbox.width(), vbox.height());

    let optionals = [surrogate_group, qt_group, hpg_group, overlap_group]
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

pub fn layout_to_svg_2(layout: &Layout, options: SvgDrawOptions) -> Document {
    let internal_bin = &layout.bin;
    let inv_bin_transf = internal_bin.pretransform.clone().inverse();
    let bin = parser::pretransform_bin(internal_bin, &inv_bin_transf);

    let vbox = bin.bbox().clone().scale(1.05);

    let theme = &options.theme;

    let stroke_width =
        fsize::min(vbox.width(), vbox.height()) * 0.001 * theme.stroke_width_multiplier;

    //draw bin
    let bin_group = {
        let mut bin_group = Group::new().set("id", format!("bin_{}", bin.id));
        let bbox = bin.bbox();
        let title = Title::new(format!(
            "bin, id: {}, bbox: [x_min: {:.3}, y_min: {:.3}, x_max: {:.3}, y_max: {:.3}]",
            bin.id, bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max
        ));

        //outer
        bin_group = bin_group
            .add(svg_export::data_to_path(
                svg_export::simple_polygon_data(&bin.outer),
                &[
                    ("fill", &*format!("{}", theme.bin_fill)),
                    ("stroke", "black"),
                    ("stroke-width", &*format!("{}", 2.0 * stroke_width)),
                ],
            ))
            .add(title);

        //holes
        for (hole_idx, hole) in bin.holes.iter().enumerate() {
            bin_group = bin_group.add(
                svg_export::data_to_path(
                    svg_export::simple_polygon_data(hole),
                    &[
                        ("fill", &*format!("{}", theme.hole_fill)),
                        ("stroke", "black"),
                        ("stroke-width", &*format!("{}", 1.0 * stroke_width)),
                    ],
                )
                .add(Title::new(format!("hole #{}", hole_idx))),
            );
        }
        bin_group
    };

    let qz_group = {
        let mut qz_group = Group::new().set("id", "quality_zones");

        //quality zones
        for qz in bin.quality_zones.iter().rev().flatten() {
            let color = theme.qz_fill[qz.quality];
            let stroke_color = svg_util::change_brightness(color, 0.5);
            for qz_shape in qz.zones.iter() {
                qz_group = qz_group.add(
                    svg_export::data_to_path(
                        svg_export::simple_polygon_data(qz_shape),
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
        for pi in layout.placed_items().values() {
            let shape = pi.shape.as_ref();
            let color = theme.item_fill.to_owned();

            let def_id = format!(
                "id_{}_t_{:.3}_{:.3}_r_{:.3}",
                pi.item_id,
                pi.d_transf.translation().0,
                pi.d_transf.translation().1,
                pi.d_transf.rotation()
            );

            item_defs = item_defs.add(Group::new().set("id", format!("item_{}", def_id)).add(
                svg_export::data_to_path(
                    svg_export::simple_polygon_data(shape),
                    &[
                        ("fill", &*format!("{}", color)),
                        ("stroke-width", &*format!("{}", stroke_width)),
                        ("fill-rule", "nonzero"),
                        ("stroke", "black"),
                        ("opacity", "0.9"),
                    ],
                ),
            ));

            if options.surrogate {
                let mut surrogate_group = Group::new().set("id", format!("surrogate_{}", def_id));
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

                let surrogate = pi.shape.surrogate();
                let poi = &surrogate.poles[0];
                let ff_poles = surrogate.ff_poles();

                for pole in surrogate.poles.iter() {
                    if pole == poi {
                        surrogate_group = surrogate_group.add(svg_export::circle(pole, &poi_style));
                    }
                    if ff_poles.contains(pole) {
                        surrogate_group = surrogate_group.add(svg_export::circle(pole, &ff_style));
                    } else {
                        surrogate_group =
                            surrogate_group.add(svg_export::circle(pole, &no_ff_style));
                    }
                }
                for pier in &surrogate.piers {
                    surrogate_group = surrogate_group.add(svg_export::data_to_path(
                        svg_export::edge_data(pier),
                        &ff_style,
                    ));
                }
                surrogate_defs = surrogate_defs.add(surrogate_group)
            }
        }
        let mut items_group = Group::new().set("id", "items").add(item_defs);
        let mut surrogate_group = Group::new().set("id", "surrogates").add(surrogate_defs);

        for pi in layout.placed_items().values() {
            let abs_transf = Transformation::empty();
            let title = Title::new(format!(
                "item, id: {}, transf: [{}]",
                pi.item_id,
                abs_transf.decompose()
            ));

            let def_id = format!(
                "id_{}_t_{:.3}_{:.3}_r_{:.3}",
                pi.item_id,
                pi.d_transf.translation().0,
                pi.d_transf.translation().1,
                pi.d_transf.rotation()
            );

            let pi_ref = Use::new()
                .set("transform", transform_to_svg(&abs_transf))
                .set("xlink:href", format!("#item_{}", def_id))
                .add(title);

            items_group = items_group.add(pi_ref);

            if options.surrogate {
                let pi_surr_ref = Use::new()
                    .set("transform", transform_to_svg(&abs_transf))
                    .set("xlink:href", format!("#surrogate_{}", def_id));

                surrogate_group = surrogate_group.add(pi_surr_ref);
            }
        }

        match options.surrogate {
            false => (items_group, None),
            true => (items_group, Some(surrogate_group)),
        }
    };

    let qt_group = match options.quadtree {
        false => None,
        true => {
            let qt_data = svg_export::quad_tree_data(layout.cde().quadtree(), &[]);
            let qt_group = Group::new()
                .set("id", "quadtree")
                .set("transform", transform_to_svg(&inv_bin_transf))
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

    let hpg_group = match options.haz_prox_grid {
        false => None,
        true => {
            let mut hpg_group = Group::new()
                .set("id", "haz_prox_grid")
                .set("transform", transform_to_svg(&inv_bin_transf));
            let hpg = layout.cde().haz_prox_grid().unwrap();
            for hp_cell in hpg.grid.cells.iter().flatten() {
                let center = hp_cell.centroid;
                let prox = hp_cell.hazard_proximity(None);
                let color = if prox == 0.0 { "red" } else { "blue" };

                hpg_group = hpg_group
                    .add(svg_export::point(center, Some(color), Some(stroke_width)))
                    .add(svg_export::circle(
                        &Circle::new(center, prox),
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

    let overlap_group = match options.highlight_overlap {
        false => None,
        true => {
            let mut overlap_group = Group::new()
                .set("id", "overlap_lines")
                .set("transform", transform_to_svg(&inv_bin_transf));
            for pi in layout.placed_items().values() {
                let collides_with = layout
                    .cde()
                    .collect_poly_collisions(pi.shape.as_ref(), &[pi.into()]);
                for haz_entity in collides_with {
                    match haz_entity {
                        HazardEntity::PlacedItem { .. } => {
                            let haz_hash = {
                                let mut hasher = DefaultHasher::new();
                                haz_entity.hash(&mut hasher);
                                hasher.finish()
                            };
                            let pi_hash = {
                                let mut hasher = DefaultHasher::new();
                                HazardEntity::from(pi).hash(&mut hasher);
                                hasher.finish()
                            };

                            if haz_hash < pi_hash {
                                // avoid duplicate lines
                                let start = pi.shape.poi.center;
                                let colliding_pkey = layout
                                    .hazard_to_p_item_key(&haz_entity)
                                    .expect("collding placed item not found");
                                let end = layout.placed_items[colliding_pkey].shape.poi.center;
                                overlap_group = overlap_group.add(svg_export::data_to_path(
                                    svg_export::edge_data(&Edge { start, end }),
                                    &[
                                        ("stroke", "lime"),
                                        ("stroke-width", &*format!("{}", stroke_width * 2.0)),
                                        (
                                            "stroke-dasharray",
                                            &*format!("{} {}", stroke_width, 4.0 * stroke_width),
                                        ),
                                        ("stroke-linecap", "round"),
                                        ("stroke-linejoin", "round"),
                                    ],
                                ));
                            }
                        }
                        HazardEntity::BinExterior => {
                            overlap_group = overlap_group.add(svg_export::point(
                                pi.shape.poi.center,
                                Some("lime"),
                                Some(2.0 * stroke_width),
                            ));
                        }
                        _ => {
                            warn!("unexpected hazard entity");
                        }
                    }
                }
            }
            Some(overlap_group)
        }
    };

    let vbox_svg = (vbox.x_min, vbox.y_min, vbox.width(), vbox.height());

    let optionals = [surrogate_group, qt_group, hpg_group, overlap_group]
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
}

fn transform_to_svg(t: &Transformation) -> String {
    //https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/transform
    //operations are effectively applied from right to left
    let dt = t.decompose();
    let (tx, ty) = dt.translation();
    let r = dt.rotation().to_degrees();
    format!("translate({tx} {ty}), rotate({r})")
}
