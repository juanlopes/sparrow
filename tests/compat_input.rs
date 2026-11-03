//! The JSON and CLI contract used before sparrow 0.3.0 still loads, and the
//! JSON we write back still uses that shape.

use jagua_rs::io::ext_repr::{ExtLayout, ExtPlacedItem, ExtTransformation};
use jagua_rs::io::import::Importer;
use jagua_rs::probs::spp::io::ext_repr::ExtSPSolution;
use sparrow::config::DEFAULT_SPARROW_CONFIG;
use sparrow::util::io::{self, DEFAULT_DPI};

const LEGACY: &str = r#"{
  "name": "legacy",
  "strip_height": 20.0,
  "items": [
    {
      "id": 5,
      "demand": 1,
      "dxf": "ignored.dxf",
      "allowed_orientations": [0.0, 180.0],
      "shape": {"type": "rectangle", "data": {"x_min": 0.0, "y_min": 0.0, "width": 4.0, "height": 2.0}},
      "min_quality": null
    },
    {
      "id": 11,
      "demand": 2,
      "shape": {"type": "rectangle", "data": {"x_min": 0.0, "y_min": 0.0, "width": 3.0, "height": 3.0}}
    }
  ]
}"#;

#[test]
fn legacy_orientations_map_to_the_old_rotation_rules() {
    let continuous = io::parse_spp_input(
        r#"{
            "name": "open",
            "strip_height": 10.0,
            "items": [{
                "id": 0,
                "demand": 1,
                "shape": {"type": "rectangle", "data": {"x_min": 0, "y_min": 0, "width": 1, "height": 1}}
            }]
        }"#,
    )
    .unwrap();
    assert!(matches!(
        continuous.instance.items[0].base.orientation.rotation,
        jagua_rs::io::ext_repr::ExtRotation::Continuous {}
    ));

    let fixed = io::parse_spp_input(
        r#"{
            "name": "fixed",
            "strip_height": 10.0,
            "items": [{
                "id": 0,
                "demand": 1,
                "allowed_orientations": [],
                "shape": {"type": "rectangle", "data": {"x_min": 0, "y_min": 0, "width": 1, "height": 1}}
            }]
        }"#,
    )
    .unwrap();
    match &fixed.instance.items[0].base.orientation.rotation {
        jagua_rs::io::ext_repr::ExtRotation::Discrete { angles } => {
            assert_eq!(angles, &vec![0.0]);
        }
        other => panic!("empty allowed_orientations must be a fixed part, got {other:?}"),
    }

    let zero = io::parse_spp_input(
        r#"{
            "name": "zero",
            "strip_height": 10.0,
            "items": [{
                "id": 0,
                "demand": 1,
                "allowed_orientations": [0],
                "shape": {"type": "rectangle", "data": {"x_min": 0, "y_min": 0, "width": 1, "height": 1}}
            }]
        }"#,
    )
    .unwrap();
    match &zero.instance.items[0].base.orientation.rotation {
        jagua_rs::io::ext_repr::ExtRotation::Discrete { angles } => {
            assert_eq!(angles, &vec![0.0]);
        }
        other => panic!("[0] must stay fixed, got {other:?}"),
    }
}

#[test]
fn legacy_payload_keeps_nonconsecutive_ids_and_historical_output_shape() {
    let mut loaded = io::parse_spp_input(LEGACY).unwrap();
    match &loaded.instance.items[0].base.orientation.rotation {
        jagua_rs::io::ext_repr::ExtRotation::Discrete { angles } => {
            assert_eq!(angles, &vec![0.0, 180.0]);
        }
        other => panic!("expected discrete 0/180, got {other:?}"),
    }
    assert_eq!(loaded.instance.items[0].base.id, 5);
    assert_eq!(loaded.instance.items[1].base.id, 11);
    assert_eq!(loaded.instance.min_item_separation, 0.0);

    let config = DEFAULT_SPARROW_CONFIG;
    let importer = Importer::new(
        config.cde_config,
        config.poly_simpl_tolerance,
        config.narrow_concavity_cutoff_ratio,
    );
    let instance = jagua_rs::probs::spp::io::import_instance(&importer, &loaded.instance)
        .expect("non-consecutive ids must import");
    assert_eq!(instance.item(0).external_id(), 5);
    assert_eq!(instance.item(1).external_id(), 11);

    let exported = ExtSPSolution {
        strip_width: 8.0,
        layout: ExtLayout {
            container_id: 0,
            placed_items: vec![ExtPlacedItem {
                item_id: 5,
                transformation: ExtTransformation {
                    reflected: false,
                    rotation: 180.0,
                    translation: (1.5, 2.5),
                },
            }],
            density: 0.25,
        },
        density: 0.25,
        run_time_sec: 3,
    };
    let output = io::fork_output_value(&loaded, &exported).unwrap();
    assert!(output.get("orientation").is_none());
    assert!(output.get("min_item_separation").is_none());
    let item = &output["items"][0];
    assert_eq!(item["allowed_orientations"], serde_json::json!([0.0, 180.0]));
    assert!(item.get("orientation").is_none());
    assert!(item.get("dxf").is_none(), "unknown metadata is dropped, as before");
    assert_eq!(item["id"], 5);
    assert_eq!(item["demand"], 1);
    let rotation = output["solution"]["layout"]["placed_items"][0]["transformation"]["rotation"]
        .as_f64()
        .unwrap();
    assert!((rotation - std::f64::consts::PI).abs() < 1e-5, "rotation {rotation} should be radians");
    assert!(output["solution"]["layout"]["placed_items"][0]["transformation"]
        .get("reflected")
        .is_none());

    // Re-reading our own output interprets radians and restores degrees for jagua.
    let text = serde_json::to_string(&output).unwrap();
    let reread = io::parse_spp_input(&text).unwrap();
    let warm = reread.solution.expect("solution round-trip");
    let warm_rotation = warm.layout.placed_items[0].transformation.rotation;
    assert!((warm_rotation - 180.0).abs() < 1e-3, "warm start rotation {warm_rotation}");

    loaded.instance.min_item_separation = 1.25;
    loaded.apply_min_separation(4.5);
    assert_eq!(loaded.instance.min_item_separation, 4.5);
    let overridden = io::fork_output_value(&loaded, &exported).unwrap();
    assert!(
        overridden.get("min_item_separation").is_none(),
        "CLI separation is not added to a legacy file that did not have the field"
    );
}

#[test]
fn json_separation_is_kept_and_cli_override_replaces_it() {
    let mut loaded = io::parse_spp_input(
        r#"{
            "name": "gapped",
            "min_item_separation": 1.5,
            "strip_height": 10.0,
            "items": [{
                "id": 0,
                "demand": 1,
                "allowed_orientations": [0, 90],
                "shape": {"type": "rectangle", "data": {"x_min": 0, "y_min": 0, "width": 1, "height": 1}}
            }]
        }"#,
    )
    .unwrap();
    assert!((loaded.instance.min_item_separation - 1.5).abs() < 1e-6);
    loaded.apply_min_separation(2.5);
    assert!((loaded.instance.min_item_separation - 2.5).abs() < 1e-6);
    let output = io::fork_output_value(
        &loaded,
        &ExtSPSolution {
            strip_width: 1.0,
            layout: ExtLayout {
                container_id: 0,
                placed_items: vec![],
                density: 0.0,
            },
            density: 0.0,
            run_time_sec: 0,
        },
    )
    .unwrap();
    assert!((output["min_item_separation"].as_f64().unwrap() - 2.5).abs() < 1e-6);
}

#[test]
fn current_orientation_payload_still_parses() {
    let loaded = io::parse_spp_input(
        r#"{
            "name": "current",
            "strip_height": 10.0,
            "items": [{
                "id": 4,
                "demand": 1,
                "orientation": {"rotation": {"mode": "stepped", "step": 90}},
                "shape": {"type": "rectangle", "data": {"x_min": 0, "y_min": 0, "width": 2, "height": 2}}
            }]
        }"#,
    )
    .unwrap();
    assert!(matches!(
        loaded.instance.items[0].base.orientation.rotation,
        jagua_rs::io::ext_repr::ExtRotation::Stepped { step } if step == 90.0
    ));
    let output = io::fork_output_value(
        &loaded,
        &ExtSPSolution {
            strip_width: 2.0,
            layout: ExtLayout {
                container_id: 0,
                placed_items: vec![],
                density: 1.0,
            },
            density: 1.0,
            run_time_sec: 0,
        },
    )
    .unwrap();
    assert!(output["items"][0].get("orientation").is_some());
    assert!(output["items"][0].get("allowed_orientations").is_none());
}

#[test]
fn dpi_default_is_real_dpi_and_legacy_value_is_the_old_default() {
    assert_eq!(io::effective_dpi(None, false), DEFAULT_DPI);
    let legacy = io::effective_dpi(None, true);
    assert!((legacy - io::legacy_dpi()).abs() < f32::EPSILON);
    assert!((io::effective_dpi(Some(300.0), true) - 300.0).abs() < f32::EPSILON);

    let fixed = io::mm_to_internal_units(3.0, DEFAULT_DPI);
    let historical = io::mm_to_internal_units(3.0, legacy);
    assert!((fixed - 3.0 * 96.0 / 25.4).abs() < 1e-4);
    assert!((historical - 0.4464).abs() < 1e-3);
    assert!(fixed > historical * 20.0, "the old default double-divided by 25.4");
}
