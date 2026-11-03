//! The global time limit is a hard stop, and the best layout found is still written.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[test]
fn cli_stops_at_the_time_limit_and_writes_the_best_solution() {
    let limit_secs = 4_u64;
    let dir = std::env::temp_dir().join(format!("sparrow-time-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("tiny.json");
    std::fs::write(
        &input,
        r#"{
            "name": "time_limit_probe",
            "strip_height": 10.0,
            "items": [{
                "id": 0,
                "demand": 6,
                "allowed_orientations": [0.0, 90.0],
                "shape": {"type": "simple_polygon", "data": [[0,0],[6,0],[6,2],[4,2],[4,1],[2,1],[2,2],[0,2]]},
                "min_quality": null
            }]
        }"#,
    )
    .unwrap();

    let binary = env!("CARGO_BIN_EXE_sparrow");
    let started = Instant::now();
    let output = Command::new(binary)
        .arg("-i")
        .arg(&input)
        .arg("-t")
        .arg(limit_secs.to_string())
        .arg("-s")
        .arg("1")
        .arg("--workers")
        .arg("1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("spawn sparrow");
    let elapsed = started.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "sparrow exited {}: {stderr}\n{stdout}",
        output.status
    );

    let json_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("output/final_time_limit_probe.json");
    let written = std::fs::read_to_string(&json_path)
        .unwrap_or_else(|err| panic!("missing {json_path:?}: {err}\n{stderr}"));
    let document: serde_json::Value = serde_json::from_str(&written).expect("solution json");
    let width = document["solution"]["strip_width"]
        .as_f64()
        .expect("strip_width");
    assert!(width.is_finite() && width > 0.0, "width {width}");
    assert!(
        document["items"][0].get("allowed_orientations").is_some(),
        "legacy input must be echoed with allowed_orientations"
    );
    assert_eq!(
        document["solution"]["layout"]["placed_items"]
            .as_array()
            .map(Vec::len),
        Some(6),
        "every demanded copy is in the written solution"
    );

    // 80/20 of 4s is the budget. A broken limiter that still uses 80%+80%, or
    // that ignores the deadline inside separation, runs well past this.
    let slack = std::time::Duration::from_millis(1500);
    assert!(
        elapsed <= std::time::Duration::from_secs(limit_secs) + slack,
        "run took {elapsed:?}, limit {limit_secs}s + {slack:?}\n{stderr}"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(2500),
        "run returned in {elapsed:?}, before the exploration budget was used\n{stderr}"
    );
}
