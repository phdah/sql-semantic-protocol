//! Independent TASK-177 DuckDB source/terminal oracles for additional original SQL.
//!
//! This is physical execution evidence, not proof of typed canonical domains,
//! adapter equivalence or vendor/session portability. Expected result bags are
//! fixed literals, not generated from the query output under test.

use duckdb::Connection;
use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const RELATIONAL: &str = include_str!("../docs/task-177-relational-bag-oracles.json");
const ORACLES: &str = include_str!("../docs/task-177-native-expansion.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("task-owned evidence")
}

fn sample_sql<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("TASK-66 original SQL")
        .iter()
        .find(|case| case["id"] == id)
        .expect("case belongs to TASK-66")["sql"]
        .as_str()
        .expect("exact original SQL")
}

fn expected_rows(source: &Value) -> Vec<Vec<Option<i64>>> {
    serde_json::from_value(source.clone()).expect("literal nullable integer rows")
}

fn rows(db: &Connection, sql: &str, columns: usize) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db
        .prepare(sql)
        .expect("original SQL accepted by native DuckDB");
    let mapped = stmt
        .query_map([], |row| {
            (0..columns)
                .map(|i| row.get::<usize, Option<i64>>(i))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("native query produces rows");
    mapped
        .map(|row| row.expect("native typed physical row"))
        .collect()
}

#[test]
fn additional_original_sql_cases_have_complete_source_and_terminal_bag_oracles() {
    let oracle = read(ORACLES);
    let original = read(SAMPLES);
    let relational = read(RELATIONAL);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        relational["source_setup"]
            .as_str()
            .expect("complete source fixture"),
    )
    .expect("native complete source setup");

    for (relation, columns) in [("orders", 2), ("customers", 2), ("products", 1)] {
        let mut actual = rows(&db, &format!("SELECT * FROM {relation}"), columns);
        let mut expected = expected_rows(&relational["source_bags"][relation]);
        actual.sort();
        expected.sort();
        assert_eq!(
            actual, expected,
            "{relation}: all source occurrences required"
        );
    }

    let cases = oracle["bag_cases"]
        .as_array()
        .expect("literal native oracles");
    assert_eq!(cases.len(), 27);
    for case in cases {
        let id = case["id"].as_str().expect("stable variant ID");
        let sql = sample_sql(&original, id);
        let expected = expected_rows(&case["expected_terminal_bag"]);
        let columns = expected.first().map_or(1, Vec::len);
        assert!(expected.iter().all(|row| row.len() == columns));
        let mut actual = rows(&db, sql, columns);
        let mut sorted_expected = expected;
        actual.sort();
        sorted_expected.sort();
        assert_eq!(
            actual, sorted_expected,
            "{id}: full native terminal occurrence bag for original SQL"
        );
    }
}

#[test]
fn original_ordering_cases_preserve_exact_position_null_policy_and_ties() {
    let oracle = read(ORACLES);
    let samples = read(SAMPLES);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        oracle["ordering_source_setup"]
            .as_str()
            .expect("complete order source"),
    )
    .expect("native order source");

    let mut actual_source = rows(&db, "SELECT id, amount FROM orders", 2);
    let mut expected_source = expected_rows(&oracle["ordering_source_bag"]);
    actual_source.sort();
    expected_source.sort();
    assert_eq!(
        actual_source, expected_source,
        "fixed NULL and duplicate source"
    );

    let cases = oracle["ordered_cases"].as_array().expect("ordered cases");
    assert_eq!(cases.len(), 5);
    for case in cases {
        let id = case["id"].as_str().expect("original case ID");
        assert_eq!(
            rows(&db, sample_sql(&samples, id), 1),
            expected_rows(&case["expected_terminal_rows"]),
            "{id}: explicit ordered row sequence including duplicate and NULL identities"
        );
    }
}

#[test]
fn named_incompatible_arity_and_multivalued_scalar_query_fail_natively() {
    let oracle = read(ORACLES);
    let samples = read(SAMPLES);
    let relational = read(RELATIONAL);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        relational["source_setup"]
            .as_str()
            .expect("full physical sources"),
    )
    .expect("native source setup");
    let cases = oracle["native_rejected_cases"]
        .as_array()
        .expect("negative cases");
    assert_eq!(cases.len(), 2);
    for id in cases {
        let id = id.as_str().expect("stable negative variant");
        let sql = sample_sql(&samples, id);
        let result = db.prepare(sql).and_then(|mut statement| {
            statement
                .query([])
                .and_then(|mut result| result.next().map(|_| ()))
        });
        assert!(
            result.is_err(),
            "{id}: must reject the original invalid input"
        );
    }
}

#[test]
fn multi_terminal_original_sql_program_preserves_each_complete_physical_bag() {
    let oracle = read(ORACLES);
    let original = read(SAMPLES);
    let relational = read(RELATIONAL);
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        relational["source_setup"]
            .as_str()
            .expect("whole original source"),
    )
    .expect("physical initial catalog");

    let cases = oracle["multi_statement_cases"]
        .as_array()
        .expect("literal multi-terminal oracle");
    assert_eq!(cases.len(), 1);
    for case in cases {
        let id = case["id"].as_str().expect("stable SQL identity");
        let sql = sample_sql(&original, id);
        let statements: Vec<_> = sql
            .split(';')
            .map(str::trim)
            .filter(|stmt| !stmt.is_empty())
            .collect();
        let expected = case["expected_terminal_bags"]
            .as_array()
            .expect("each physical terminal");
        assert_eq!(statements.len(), 2, "{id}: two original SQL statements");
        assert_eq!(statements.len(), expected.len());
        for (index, (statement, want)) in statements.iter().zip(expected).enumerate() {
            let mut actual = rows(&db, statement, 1);
            let mut expected = expected_rows(want);
            actual.sort();
            expected.sort();
            assert_eq!(
                actual, expected,
                "{id}: independent exact SQL terminal {index} and complete bag"
            );
        }
    }
}
