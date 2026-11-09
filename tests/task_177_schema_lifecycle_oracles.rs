//! TASK-177 original DDL and transaction native catalog oracles.
//!
//! Every original SQL program is executed from a fresh, explicitly observed
//! physical catalog. Independent literal input/terminal bags and rejection
//! checks cover constraints, materialized/view lifetimes and rollback.
//! External vendor and canonical typed-domain certification remain separate.

use duckdb::Connection;
use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const ORACLES: &str = include_str!("../docs/task-177-schema-lifecycle-oracles.json");

fn read(value: &str) -> Value {
    serde_json::from_str(value).expect("task-owned versioned evidence")
}

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("all TASK-66 samples")
        .iter()
        .find(|entry| entry["id"] == id)
        .expect("exact original sample")["sql"]
        .as_str()
        .expect("original SQL text")
}

fn bag(db: &Connection, sql: &str, columns: usize) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("native source or terminal query");
    let result = stmt
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get::<usize, Option<i64>>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("physical query");
    let mut rows: Vec<_> = result
        .map(|row| row.expect("integer/NULL physical value"))
        .collect();
    rows.sort();
    rows
}

fn expected(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut rows: Vec<Vec<Option<i64>>> =
        serde_json::from_value(value.clone()).expect("literal nullable integer bag");
    rows.sort();
    rows
}

fn check(db: &Connection, query: &str, rows: &Value, context: &str) {
    let want = expected(rows);
    let width = want.first().map_or(1, Vec::len);
    assert!(want.iter().all(|row| row.len() == width));
    assert_eq!(
        bag(db, query, width),
        want,
        "{context}: every physical occurrence and NULL must be preserved"
    );
}

#[test]
fn original_schema_and_transaction_programs_match_source_terminal_and_negative_oracles() {
    let fixture = read(ORACLES);
    let samples = read(SAMPLES);
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["case_count"], 22);
    let cases = fixture["cases"].as_array().expect("task-owned SQL programs");
    assert_eq!(cases.len(), 22);

    for case in cases {
        let id = case["id"].as_str().expect("stable TASK-66 fixture");
        let db = Connection::open_in_memory().expect("native DuckDB");
        let setup = case["source_setup"].as_str().expect("independent source");
        if !setup.is_empty() {
            db.execute_batch(setup)
                .unwrap_or_else(|error| panic!("{id}: invalid source fixture: {error}"));
        }
        let before = case["expected_source_query"]
            .as_str()
            .expect("physical input probe");
        check(&db, before, &case["expected_source_bag"], id);

        db.execute_batch(original(&samples, id))
            .unwrap_or_else(|error| panic!("{id}: original SQL must run natively: {error}"));
        let after = case["after_original_sql"]
            .as_str()
            .expect("explicit follow-up mutations");
        if !after.is_empty() {
            db.execute_batch(after)
                .unwrap_or_else(|error| panic!("{id}: valid follow-up mutation failed: {error}"));
        }

        let terminal = case["expected_terminal_query"]
            .as_str()
            .expect("complete poststate query");
        check(&db, terminal, &case["expected_terminal_bag"], id);

        if let Some(rejected_sql) = case.get("must_reject_after") {
            let rejected = rejected_sql.as_str().expect("negative SQL");
            assert!(
                db.execute_batch(rejected).is_err(),
                "{id}: invalid mutation or relation lookup must fail"
            );
            check(
                &db,
                terminal,
                &case["expected_terminal_bag"],
                &format!("{id}: rejected mutation must not change poststate"),
            );
        }
    }
}
