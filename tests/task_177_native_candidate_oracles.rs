//! TASK-177 conditional original SQL native candidate probes.
//!
//! These are independently expected physical invariants for named DuckDB
//! execution, not vendor equivalence or canonical semantic signoff.

use duckdb::Connection;
use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const SOURCES: &str = include_str!("../docs/task-177-relational-bag-oracles.json");
const BOUNDARIES: &str = include_str!("../docs/task-177-native-boundary-probes.json");

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("all frozen TASK-66 samples")
        .iter()
        .find(|case| case["id"] == id)
        .expect("fixed original sample")["sql"]
        .as_str()
        .expect("exact frozen original SQL")
}

#[test]
fn original_binary_cast_is_a_named_binder_rejection_with_a_separate_blob_control() {
    let samples: Value = serde_json::from_str(SAMPLES).expect("samples");
    let db = Connection::open_in_memory().expect("native DuckDB");
    let sql = original(&samples, "88-C177-S176");
    let error = match db.prepare(sql) {
        Ok(_) => panic!("frozen BINARY(2) unexpectedly accepted"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("does not take any type parameters"),
        "original binder boundary changed: {error}"
    );
    // Companion SQL is not a proof of the original parameterized syntax.
    let mut stmt = db
        .prepare("SELECT CAST('ab' AS BLOB)")
        .expect("native BLOB");
    let mut rows = stmt.query([]).expect("native BLOB output");
    let row = rows
        .next()
        .expect("control output")
        .expect("one binary row");
    let payload: Vec<u8> = row.get(0).expect("BLOB payload");
    assert_eq!(payload.as_slice(), b"ab");
    assert!(rows.next().expect("complete companion result").is_none());
}

#[test]
fn original_geometry_cast_preserves_exact_point_coordinates() {
    let samples: Value = serde_json::from_str(SAMPLES).expect("samples");
    let db = Connection::open_in_memory().expect("native DuckDB");
    let sql = original(&samples, "88-C177-S181");
    let rendered = format!("SELECT CAST(({sql}) AS VARCHAR)");
    let mut stmt = db.prepare(&rendered).expect("original geometry scalar");
    let mut rows = stmt.query([]).expect("native geometry result");
    let row = rows.next().expect("native output").expect("one geometry");
    let wkt: String = row.get(0).expect("WKT physical representation");
    assert_eq!(wkt, "POINT (0 0)");
    assert!(rows.next().expect("complete terminal").is_none());

    let type_sql = format!("SELECT typeof(({sql}))");
    let mut type_stmt = db.prepare(&type_sql).expect("native geometry type");
    let mut type_rows = type_stmt.query([]).expect("physical type result");
    let row = type_rows.next().expect("type row").expect("one type");
    let native_type: String = row.get(0).expect("geometry type");
    assert_eq!(native_type, "GEOMETRY");
    assert!(type_rows.next().expect("complete type result").is_none());
}

#[test]
fn original_explain_has_one_plan_for_the_complete_named_source() {
    let samples: Value = serde_json::from_str(SAMPLES).expect("samples");
    let sources: Value = serde_json::from_str(SOURCES).expect("complete source fixtures");
    let boundaries: Value = serde_json::from_str(BOUNDARIES).expect("native boundaries");
    assert_eq!(
        boundaries["native_executable_without_fixed_terminal"][0], "88-C177-S242",
        "EXPLAIN remains a version-sensitive plan rather than fixed terminal"
    );
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(sources["source_setup"].as_str().expect("sources"))
        .expect("physical inputs");
    let mut stmt = db
        .prepare(original(&samples, "88-C177-S242"))
        .expect("original EXPLAIN syntax");
    let mut rows = stmt.query([]).expect("EXPLAIN terminal");
    let row = rows
        .next()
        .expect("native explain result")
        .expect("one plan");
    let kind: String = row.get(0).expect("plan kind");
    let plan: String = row.get(1).expect("plan text");
    assert!(!kind.is_empty(), "plan kind is a required terminal column");
    assert!(
        plan.to_lowercase().contains("orders"),
        "plan must reference original source"
    );
    assert!(rows.next().expect("one terminal plan").is_none());
}
