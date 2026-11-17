//! TASK-179 original transaction source and terminal DuckDB oracles.
//!
//! Exact TASK-177 SQL is executed, including both the original INSERT and
//! ROLLBACK controls. Metadata adapters cannot attest transaction execution.

use duckdb::Connection;
use serde_json::Value;

const SOURCE: &str = include_str!("../docs/task-177-parser-samples.json");
const NATIVE: &str = include_str!("../docs/task-177-schema-lifecycle-oracles.json");
const MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("immutable source")
}

fn rows(db: &Connection, sql: &str) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("complete source/terminal query");
    let width = stmt.column_count();
    let output = stmt
        .query_map([], |row| {
            (0..width)
                .map(|column| row.get::<usize, Option<i64>>(column))
                .collect::<duckdb::Result<Vec<_>>>()
        })
        .expect("bounded source rows");
    let mut bag = output
        .map(|item| item.expect("native SQL row"))
        .collect::<Vec<_>>();
    bag.sort();
    bag
}

fn expected(source: &Value, key: &str) -> Vec<Vec<Option<i64>>> {
    let mut bag = serde_json::from_value(source[key].clone())
        .expect("independently frozen source bag or terminal");
    bag.sort();
    bag
}

#[test]
fn four_original_transaction_scripts_replay_full_source_and_afterstate_without_external_claims() {
    let samples = read(SOURCE);
    let oracles = read(NATIVE);
    let matrix = read(MATRIX);
    let mut verified = 0;
    for original_id in [
        "88-C177-S221",
        "88-C177-S222",
        "88-C177-S223",
        "88-C177-S224",
    ] {
        let original = samples["cases"]
            .as_array()
            .expect("original SQLs")
            .iter()
            .find(|case| case["id"] == original_id)
            .expect("original transaction source spelling");
        let oracle = oracles["cases"]
            .as_array()
            .expect("independent native oracles")
            .iter()
            .find(|case| case["id"] == original_id)
            .expect("complete original DuckDB source and afterstate");
        assert!(
            matrix["cases"]
                .as_array()
                .expect("TASK-179 cases")
                .iter()
                .any(|case| case["source_case_id"] == original_id),
            "missing TASK-179 transaction evidence owner"
        );
        let db = Connection::open_in_memory().expect("DuckDB");
        let setup = oracle["source_setup"].as_str().expect("source SQL");
        if !setup.is_empty() {
            db.execute_batch(setup).expect("original source setup");
        }
        let source_sql = oracle["expected_source_query"]
            .as_str()
            .expect("original source snapshot");
        assert_eq!(
            rows(&db, source_sql),
            expected(oracle, "expected_source_bag"),
            "{original_id}: entire native initial source population"
        );
        db.execute_batch(original["sql"].as_str().expect("unaltered original SQL"))
            .unwrap_or_else(|error| panic!("{original_id}: original native SQL: {error}"));
        let terminal_sql = oracle["expected_terminal_query"]
            .as_str()
            .expect("source-owned native terminal");
        assert_eq!(
            rows(&db, terminal_sql),
            expected(oracle, "expected_terminal_bag"),
            "{original_id}: every final SQL row occurrence"
        );
        verified += 1;
    }
    assert_eq!(verified, 4);
}
