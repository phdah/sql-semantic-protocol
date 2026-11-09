//! TASK-177 native predicates/environment physical oracles.
//!
//! Separate source fixtures keep session timezone and comparison rules fixed.
//! Exact original SQL is executed; no parsed sample implies another vendor law.

use duckdb::Connection;
use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const ORACLES: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");
const ORDERING: &str = include_str!("../docs/task-177-native-expansion.json");

fn read(text: &str) -> Value {
    serde_json::from_str(text).expect("versioned evidence")
}

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("TASK-66 sample registry")
        .iter()
        .find(|case| case["id"] == id)
        .expect("valid original ID")["sql"]
        .as_str()
        .expect("exact original SQL")
}

fn int_rows(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut statement = db.prepare(sql).expect("original native SQL");
    statement
        .query_map([], |row| row.get::<usize, Option<i64>>(0))
        .expect("native result")
        .map(|row| row.expect("typed native integer result"))
        .collect()
}

fn assert_int_bag(mut actual: Vec<Option<i64>>, expected: &Value, id: &str) {
    let want: Vec<Vec<Option<i64>>> =
        serde_json::from_value(expected.clone()).expect("literal expected bag");
    assert!(want.iter().all(|row| row.len() == 1));
    let mut want: Vec<_> = want.into_iter().map(|row| row[0]).collect();
    actual.sort();
    want.sort();
    assert_eq!(actual, want, "{id}: full terminal multiplicity");
}

#[test]
fn original_string_predicates_match_complete_nullable_duplicate_source_and_output() {
    let fixture = read(ORACLES);
    let samples = read(SAMPLES);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        fixture["customer_source_setup"]
            .as_str()
            .expect("complete input"),
    )
    .expect("native named customer source");
    let mut statement = db.prepare("SELECT id, name FROM customers").expect("source");
    let mut source: Vec<(Option<i64>, Option<String>)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("native source")
        .map(|row| row.expect("source row"))
        .collect();
    let mut expected: Vec<(Option<i64>, Option<String>)> =
        serde_json::from_value(fixture["customer_source_bag"].clone())
            .expect("complete independent source bag");
    source.sort();
    expected.sort();
    assert_eq!(source, expected, "complete string source including NULL names");

    let cases = fixture["customer_cases"].as_array().expect("five string cases");
    assert_eq!(cases.len(), 5);
    for case in cases {
        let id = case["id"].as_str().expect("fixed variant");
        assert_int_bag(
            int_rows(&db, original(&samples, id)),
            &case["expected_terminal_bag"],
            id,
        );
    }
}

#[test]
fn original_temporal_comparisons_obey_explicit_utc_session_and_source_nulls() {
    let fixture = read(ORACLES);
    let samples = read(SAMPLES);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        fixture["temporal_source_setup"]
            .as_str()
            .expect("fixed UTC and complete source"),
    )
    .expect("native timestamp source");

    let mut statement = db
        .prepare("SELECT id, strftime(created_at, '%Y-%m-%d %H:%M:%S') FROM orders")
        .expect("timestamp source");
    let mut actual: Vec<(Option<i64>, Option<String>)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("source rows")
        .map(|row| row.expect("typed timestamp row"))
        .collect();
    let mut expected: Vec<(Option<i64>, Option<String>)> =
        serde_json::from_value(fixture["temporal_source_bag"].clone())
            .expect("all source timestamps");
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected, "full timestamp source with NULL and duplicates");

    let cases = fixture["temporal_cases"].as_array().expect("four temporal cases");
    assert_eq!(cases.len(), 4);
    for case in cases {
        let id = case["id"].as_str().expect("fixed variant");
        assert_int_bag(
            int_rows(&db, original(&samples, id)),
            &case["expected_terminal_bag"],
            id,
        );
    }
}

#[test]
fn signed_zero_equality_and_explicit_null_sort_have_native_oracles() {
    let fixture = read(ORACLES);
    let samples = read(SAMPLES);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        fixture["floating_source_setup"]
            .as_str()
            .expect("fixed signed zero source"),
    )
    .expect("floating source");
    let mut stmt = db.prepare("SELECT id, amount FROM orders").expect("source");
    let mut actual: Vec<(Option<i64>, Option<f64>)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("source rows")
        .map(|row| row.expect("source value"))
        .collect();
    let mut expected: Vec<(Option<i64>, Option<f64>)> =
        serde_json::from_value(fixture["floating_source_bag"].clone())
            .expect("literal source");
    actual.sort_by(|a, b| a.0.cmp(&b.0));
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(actual, expected, "signed zeros, NULL and nonzero source");
    let case = &fixture["floating_cases"][0];
    assert_int_bag(
        int_rows(&db, original(&samples, "88-C177-S041")),
        &case["expected_terminal_bag"],
        "88-C177-S041",
    );

    let sorting = read(ORDERING);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        sorting["ordering_source_setup"]
            .as_str()
            .expect("fixed ordering source"),
    )
    .expect("ordering input");
    let case = &fixture["ordering_case"];
    let id = case["id"].as_str().expect("sort variant");
    let actual = int_rows(&db, original(&samples, id));
    let expected: Vec<Vec<Option<i64>>> =
        serde_json::from_value(case["expected_terminal_rows"].clone())
            .expect("full ordered result");
    assert_eq!(
        actual,
        expected.into_iter().map(|row| row[0]).collect::<Vec<_>>(),
        "{id}: NULLS LAST and tie-position semantics"
    );
}

#[test]
fn fixed_width_overflow_is_a_native_negative_oracle() {
    let fixture = read(ORACLES);
    let samples = read(SAMPLES);
    let db = Connection::open_in_memory().expect("DuckDB");
    let id = fixture["native_rejected_cases"][0]["id"]
        .as_str()
        .expect("original overflow");
    assert_eq!(id, "88-C177-S033");
    let sql = original(&samples, id);
    let result = db.prepare(sql).and_then(|mut statement| {
        statement
            .query([])
            .and_then(|mut rows| rows.next().map(|_| ()))
    });
    assert!(result.is_err(), "{id}: arithmetic overflow must not wrap silently");
}
