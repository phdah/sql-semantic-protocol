//! TASK-177 original deterministic SQL native differential checks.
//!
//! Every successful sample executes its exact original SQL. A scalar wrapper
//! renders non-integer values without changing the original subquery.
//! Source and terminal evidence here is only for one pinned DuckDB session.

use duckdb::Connection;
use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const EVIDENCE: &str = include_str!("../docs/task-177-final-native-deterministic.json");
const STRINGS: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");
const RELATIONS: &str = include_str!("../docs/task-177-relational-bag-oracles.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned TASK-177 evidence")
}

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("original SQL registry")
        .iter()
        .find(|case| case["id"] == id)
        .expect("original SQL case")["sql"]
        .as_str()
        .expect("exact original SQL")
}

fn native_integers(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut stmt = db.prepare(sql).expect("original native SQL");
    stmt.query_map([], |row| row.get::<usize, Option<i64>>(0))
        .expect("native integer output")
        .map(|item| item.expect("nullable integer"))
        .collect()
}

fn assert_integer_bag(db: &Connection, sql: &str, expected: &Value, id: &str) {
    let mut actual = native_integers(db, sql);
    let mut expected: Vec<Vec<Option<i64>>> =
        serde_json::from_value(expected.clone()).expect("literal full occurrence bag");
    assert!(expected.iter().all(|row| row.len() == 1));
    let mut want: Vec<_> = expected
        .drain(..)
        .map(|row| row[0])
        .collect();
    actual.sort();
    want.sort();
    assert_eq!(actual, want, "{id}: complete original SQL output bag");
}

#[test]
fn original_pattern_and_collation_sql_has_complete_literal_native_bags() {
    let evidence = read(EVIDENCE);
    let samples = read(SAMPLES);
    let strings = read(STRINGS);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(
        strings["customer_source_setup"]
            .as_str()
            .expect("complete named customers"),
    )
    .expect("native customer setup");

    let mut source = db
        .prepare("SELECT id, name FROM customers")
        .expect("whole customer source");
    let mut observed: Vec<(Option<i64>, Option<String>)> = source
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("source rows")
        .map(|row| row.expect("typed source"))
        .collect();
    let mut expected: Vec<(Option<i64>, Option<String>)> =
        serde_json::from_value(strings["customer_source_bag"].clone())
            .expect("literal customer source");
    observed.sort();
    expected.sort();
    assert_eq!(observed, expected, "complete source NULL and duplicate identities");

    for case in evidence["original_integer_selects"]
        .as_array()
        .expect("named original predicates")
    {
        if case["source"] != "customers" {
            continue;
        }
        let id = case["id"].as_str().expect("case ID");
        assert_integer_bag(
            &db,
            original(&samples, id),
            &case["expected_terminal_bag"],
            id,
        );
    }
}

#[test]
fn native_nan_comparison_retains_source_duplicates_and_non_ieee_equality_law() {
    let evidence = read(EVIDENCE);
    let samples = read(SAMPLES);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(
        evidence["nan_source_setup"].as_str().expect("typed NaN source"),
    )
    .expect("native floating input");
    assert_integer_bag(
        &db,
        "SELECT id FROM orders",
        &evidence["nan_source_id_bag"],
        "complete NaN source",
    );
    let case = evidence["original_integer_selects"]
        .as_array()
        .expect("all candidates")
        .iter()
        .find(|case| case["id"] == "88-C177-S040")
        .expect("NaN equality sample");
    assert_integer_bag(
        &db,
        original(&samples, "88-C177-S040"),
        &case["expected_terminal_bag"],
        "88-C177-S040",
    );
}

#[test]
fn original_scalar_aggregate_and_typed_values_match_independent_native_outputs() {
    let evidence = read(EVIDENCE);
    let samples = read(SAMPLES);
    let relational = read(RELATIONS);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(relational["source_setup"].as_str().expect("whole inputs"))
        .expect("source relations");

    let float_cases = evidence["original_scalar_floating"]
        .as_array()
        .expect("scalar float oracles");
    for case in float_cases {
        let id = case["id"].as_str().expect("original case ID");
        let mut stmt = db.prepare(original(&samples, id)).expect("original scalar");
        let mut rows = stmt.query([]).expect("native scalar");
        let row = rows.next().expect("native row").expect("one scalar row");
        let actual: Option<f64> = row.get(0).expect("typed aggregate");
        let expected = case["expected"].as_f64().expect("literal floating oracle");
        assert_eq!(actual, Some(expected), "{id}: exact mean");
        assert!(rows.next().expect("one complete result").is_none());
    }

    for case in evidence["original_scalar_rendered"]
        .as_array()
        .expect("literal strongly typed scalar cases")
    {
        let id = case["id"].as_str().expect("original typed scalar");
        let sql = original(&samples, id);
        // The exact original SQL appears unchanged as the scalar subquery.
        let rendered = format!("SELECT CAST(({sql}) AS VARCHAR)");
        let mut stmt = db.prepare(&rendered).expect("native typed scalar");
        let mut rows = stmt.query([]).expect("native scalar cast");
        let row = rows.next().expect("native result row").expect("one scalar");
        let actual: String = row.get(0).expect("rendered scalar");
        assert_eq!(
            actual,
            case["expected"].as_str().expect("literal expected value"),
            "{id}: typed value must match independent oracle"
        );
        assert!(rows.next().expect("one terminal row").is_none());
    }
}

#[test]
fn original_enum_and_timezone_statements_have_explicit_native_session_poststates() {
    let evidence = read(EVIDENCE);
    let samples = read(SAMPLES);
    let cases = evidence["original_catalog_state"]
        .as_array()
        .expect("DDL and session original SQL");
    assert_eq!(cases.len(), 2);
    for case in cases {
        let id = case["id"].as_str().expect("original case ID");
        let db = Connection::open_in_memory().expect("native DuckDB");
        db.execute_batch(original(&samples, id))
            .unwrap_or_else(|error| panic!("{id}: original state mutation: {error}"));
        let sql = case["poststate_sql"]
            .as_str()
            .expect("physical poststate observation");
        let mut stmt = db.prepare(sql).expect("native poststate");
        let mut rows = stmt.query([]).expect("native poststate read");
        let row = rows.next().expect("native row").expect("one poststate");
        let actual: String = row.get(0).expect("poststate value");
        assert_eq!(
            actual,
            case["expected"].as_str().expect("independently expected state"),
            "{id}: original SQL must produce pinned native state"
        );
        assert!(rows.next().expect("one state row").is_none());
    }
}
