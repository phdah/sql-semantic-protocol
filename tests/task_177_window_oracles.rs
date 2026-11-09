//! TASK-177 native independent ranking and frame-wide complete-bag oracles.

use duckdb::Connection;
use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const RELATIONAL: &str = include_str!("../docs/task-177-relational-bag-oracles.json");
const ORACLES: &str = include_str!("../docs/task-177-window-oracles.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned evidence")
}

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("original TASK-66 registry")
        .iter()
        .find(|sample| sample["id"] == id)
        .expect("sample ID")["sql"]
        .as_str()
        .expect("original SQL")
}

#[test]
fn native_window_frames_and_rankings_match_full_duplicate_null_source_bags() {
    let fixture = read(ORACLES);
    let samples = read(SAMPLES);
    let relational = read(RELATIONAL);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        relational["source_setup"]
            .as_str()
            .expect("full original source"),
    )
    .expect("native input catalog");

    let mut source = db
        .prepare("SELECT id, amount FROM orders")
        .expect("physical input");
    let mut actual_source: Vec<Vec<Option<i64>>> = source
        .query_map([], |row| Ok(vec![row.get(0)?, row.get(1)?]))
        .expect("source rows")
        .map(|row| row.expect("typed row"))
        .collect();
    let mut expected_source: Vec<Vec<Option<i64>>> =
        serde_json::from_value(relational["source_bags"]["orders"].clone())
            .expect("independent source bag");
    actual_source.sort();
    expected_source.sort();
    assert_eq!(actual_source, expected_source, "full input including ties and NULL");

    let integers = fixture["integer_cases"]
        .as_array()
        .expect("four integer-result windows");
    assert_eq!(integers.len(), 4);
    for case in integers {
        let id = case["id"].as_str().expect("original case ID");
        let mut stmt = db.prepare(original(&samples, id)).expect("native window");
        let values = stmt
            .query_map([], |row| row.get::<usize, Option<i64>>(0))
            .expect("window rows");
        let mut actual: Vec<Option<i64>> =
            values.map(|row| row.expect("typed integer")).collect();
        let mut expected: Vec<Option<i64>> =
            serde_json::from_value(case["expected_terminal_bag"].clone())
                .expect("independent nullable integers");
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{id}: full window result bag");
    }

    let floating = fixture["float_cases"]
        .as_array()
        .expect("three numeric ranking/frame windows");
    assert_eq!(floating.len(), 3);
    for case in floating {
        let id = case["id"].as_str().expect("original case ID");
        let mut stmt = db.prepare(original(&samples, id)).expect("native ranking");
        let values = stmt
            .query_map([], |row| row.get::<usize, Option<f64>>(0))
            .expect("native ranking rows");
        let mut actual: Vec<Option<f64>> =
            values.map(|row| row.expect("typed numeric")).collect();
        let mut expected: Vec<Option<f64>> =
            serde_json::from_value(case["expected_terminal_bag"].clone())
                .expect("independent fractional/nullable bag");
        actual.sort_by(|a, b| a.partial_cmp(b).expect("finite native values"));
        expected.sort_by(|a, b| a.partial_cmp(b).expect("finite frozen values"));
        assert_eq!(actual.len(), expected.len(), "{id}: result multiplicity");
        for (observed, want) in actual.iter().zip(expected) {
            match (observed, want) {
                (None, None) => {}
                (Some(actual), Some(want)) => assert!(
                    (actual - want).abs() < 1e-9,
                    "{id}: numerical ranking/window result {actual} differs from {want}"
                ),
                _ => panic!("{id}: NULL distinction lost"),
            }
        }
    }
}
