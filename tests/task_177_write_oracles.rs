//! TASK-177 native DML original SQL pre/post-state oracles.
//!
//! Each original statement is executed on a fresh complete physical catalog.
//! Literal expected bags are independent of the observed output and retain
//! duplicate target/source occurrences and NULL-valued rows.

use duckdb::Connection;
use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const ORACLES: &str = include_str!("../docs/task-177-write-oracles.json");

fn read(src: &str) -> Value {
    serde_json::from_str(src).expect("versioned literal fixtures")
}

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("sample inventory")
        .iter()
        .find(|case| case["id"] == id)
        .expect("original sample required")["sql"]
        .as_str()
        .expect("original SQL")
}

fn expected(value: &Value) -> Vec<Vec<Option<i64>>> {
    serde_json::from_value(value.clone()).expect("complete typed nullable rows")
}

fn rows(db: &Connection, sql: &str, columns: usize) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("native statement");
    let result = stmt
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get::<usize, Option<i64>>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("native DML result");
    result
        .map(|row| row.expect("typed physical value"))
        .collect()
}

fn assert_bag(mut actual: Vec<Vec<Option<i64>>>, mut want: Vec<Vec<Option<i64>>>, context: &str) {
    actual.sort();
    want.sort();
    assert_eq!(actual, want, "{context}: full physical bag");
}

#[test]
fn original_dml_writes_match_complete_initial_and_terminal_duckdb_bags() {
    let fixture = read(ORACLES);
    let samples = read(SAMPLES);
    let cases = fixture["cases"].as_array().expect("native DML cases");
    assert_eq!(cases.len(), 16);

    for case in cases {
        let id = case["id"].as_str().expect("stable case ID");
        let db = Connection::open_in_memory().expect("native DuckDB");
        let primary_key = case["source"] == "primary_key";
        let setup = if primary_key {
            fixture["primary_key_source_setup"]
                .as_str()
                .expect("complete PK source")
        } else {
            fixture["source_setup"]
                .as_str()
                .expect("complete duplicate/NULL sources")
        };
        db.execute_batch(setup)
            .expect("complete native initial catalog");
        let initial = if primary_key {
            vec![vec![Some(1), Some(10)], vec![Some(2), Some(20)], vec![Some(3), None]]
        } else {
            expected(&fixture["orders_source_bag"])
        };
        assert_bag(rows(&db, "SELECT id, amount FROM orders", 2), initial, id);
        if !primary_key {
            assert_bag(
                rows(&db, "SELECT id, amount FROM customers", 2),
                expected(&fixture["customers_source_bag"]),
                id,
            );
        }

        let sql = original(&samples, id);
        if let Some(want) = case.get("expected_returning_bag") {
            assert_bag(rows(&db, sql, 1), expected(want), id);
        } else {
            db.execute_batch(sql).unwrap_or_else(|error| {
                panic!("{id}: original DML must execute natively: {error}")
            });
        }
        assert_bag(
            rows(&db, "SELECT id, amount FROM orders", 2),
            expected(&case["expected_terminal_bag"]),
            id,
        );
    }
}
