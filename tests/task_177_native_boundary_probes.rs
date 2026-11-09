//! TASK-177 bounded original SQL probes for native rejection and nondeterminism.
//!
//! A rejected syntax, conditional capability, or stochastic result is not a
//! successful full-terminal oracle. These tests use one complete DuckDB source
//! and preserve that evidence boundary rather than fabricating exact results.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const BOUNDARIES: &str = include_str!("../docs/task-177-native-boundary-probes.json");
const RELATIONS: &str = include_str!("../docs/task-177-relational-bag-oracles.json");

fn parse(value: &str) -> Value {
    serde_json::from_str(value).expect("frozen TASK-177 registry")
}

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    samples["cases"]
        .as_array()
        .expect("all SQL variants")
        .iter()
        .find(|case| case["id"] == id)
        .expect("original TASK-66 variant")["sql"]
        .as_str()
        .expect("exact original SQL")
}

fn source_db() -> Connection {
    let db = Connection::open_in_memory().expect("native DuckDB");
    let sources = parse(RELATIONS);
    db.execute_batch(
        sources["source_setup"]
            .as_str()
            .expect("complete source setup"),
    )
    .expect("physical source setup");
    db
}

fn native_ids(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut statement = db.prepare(sql).expect("native SQL parses");
    statement
        .query_map([], |row| row.get::<usize, Option<i64>>(0))
        .expect("native SQL executes")
        .map(|row| row.expect("typed physical occurrence"))
        .collect()
}

fn complete_native_sources(db: &Connection) -> BTreeMap<String, Vec<Vec<Option<i64>>>> {
    let fixture = parse(RELATIONS);
    let mut observed = BTreeMap::new();
    for (relation, column_count) in [("orders", 2), ("customers", 2), ("products", 1)] {
        let mut statement = db
            .prepare(&format!("SELECT * FROM {relation}"))
            .expect("named physical source");
        let mut rows: Vec<Vec<Option<i64>>> = statement
            .query_map([], |row| {
                (0..column_count)
                    .map(|index| row.get::<usize, Option<i64>>(index))
                    .collect::<duckdb::Result<Vec<_>>>()
            })
            .expect("complete physical source bag")
            .map(|row| row.expect("typed physical row"))
            .collect();
        let mut expected: Vec<Vec<Option<i64>>> =
            serde_json::from_value(fixture["source_bags"][relation].clone())
                .expect("independent source occurrence bag");
        rows.sort();
        expected.sort();
        assert_eq!(rows, expected, "{relation}: source fixture drift");
        observed.insert(relation.to_string(), rows);
    }
    observed
}

fn occurrence_counts(rows: &[Option<i64>]) -> BTreeMap<Option<i64>, usize> {
    let mut counts = BTreeMap::new();
    for &id in rows {
        *counts.entry(id).or_insert(0) += 1;
    }
    counts
}

#[test]
fn frozen_native_rejections_are_observed_with_complete_source_relations() {
    let boundaries = parse(BOUNDARIES);
    let samples = parse(SAMPLES);
    let rejected = boundaries["expected_native_rejections"]
        .as_array()
        .expect("named and reviewed exception candidates");
    assert_eq!(rejected.len(), 12);
    let mut unexpectedly_accepted = Vec::new();
    for case in rejected {
        let id = case["id"].as_str().expect("original ID");
        let db = source_db();
        let prestate = complete_native_sources(&db);
        let sql = original(&samples, id);
        // SELECT must be materialized: execute_batch alone can succeed even
        // when the deferred expression would fail at query time.
        let result: duckdb::Result<()> = if sql.trim_start().starts_with("SELECT") {
            db.prepare(sql).and_then(|mut statement| {
                let mut rows = statement.query([])?;
                while rows.next()?.is_some() {}
                Ok(())
            })
        } else {
            db.execute_batch(sql)
        };
        if result.is_ok() {
            unexpectedly_accepted.push(id);
        }
        assert_eq!(
            complete_native_sources(&db),
            prestate,
            "{id}: failed native statement must not mutate any source bag"
        );
    }
    assert!(
        unexpectedly_accepted.is_empty(),
        "original SQL unexpectedly executed: {unexpectedly_accepted:?}"
    );
}

#[test]
fn stochastic_native_samples_never_invent_or_amplify_source_occurrences() {
    let boundaries = parse(BOUNDARIES);
    let samples = parse(SAMPLES);
    let db = source_db();
    let source = native_ids(&db, "SELECT id FROM orders");
    let source_counts = occurrence_counts(&source);
    assert_eq!(source.len(), 6, "all duplicate and NULL inputs retained");

    let cases = boundaries["stochastic_sampling_cases"]
        .as_array()
        .expect("bounded sampling examples");
    assert_eq!(cases.len(), 5);
    for case in cases {
        let id = case.as_str().expect("original sample ID");
        let sql = original(&samples, id);
        let executable = match db.prepare(sql) {
            Ok(_) => sql.to_string(),
            Err(error) => {
                assert!(
                    error.to_string().to_lowercase().contains("sample"),
                    "{id}: unexpected native parser error: {error}"
                );
                let percent = sql.replace("(10)", "(10 PERCENT)");
                assert_ne!(sql, percent, "{id}: explicit percent companion required");
                percent
            }
        };
        let output = native_ids(&db, &executable);
        for (value, count) in occurrence_counts(&output) {
            assert!(
                count <= source_counts.get(&value).copied().unwrap_or(0),
                "{id}: sampling must be an occurrence sub-bag, including NULL"
            );
        }
    }
}

#[test]
fn bounded_recursive_companion_has_exact_rows_without_running_infinite_original() {
    let boundaries = parse(BOUNDARIES);
    let samples = parse(SAMPLES);
    let id = boundaries["nonterminating_case"]
        .as_str()
        .expect("original recursive sample");
    let sql = original(&samples, id);
    assert!(sql.contains("WITH RECURSIVE"));
    assert!(!sql.contains("WHERE n <"), "original remains unbounded");
    let db = source_db();
    let mut actual = native_ids(
        &db,
        boundaries["bounded_companion"]
            .as_str()
            .expect("terminating explicit control"),
    );
    let mut expected: Vec<Option<i64>> = serde_json::from_value::<Vec<Vec<Option<i64>>>>(
        boundaries["bounded_companion_bag"].clone(),
    )
    .expect("independent expected control bag")
    .into_iter()
    .map(|row| {
        assert_eq!(row.len(), 1);
        row[0]
    })
    .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn random_original_has_bounded_values_but_no_fabricated_fixed_terminal_bag() {
    let boundaries = parse(BOUNDARIES);
    let samples = parse(SAMPLES);
    let id = boundaries["runtime_random_case"]
        .as_str()
        .expect("original nondeterministic example");
    let db = source_db();
    let mut statement = db.prepare(original(&samples, id)).expect("RANDOM SQL");
    let values: Vec<f64> = statement
        .query_map([], |row| row.get(0))
        .expect("physical random stream")
        .map(|value| value.expect("floating sample"))
        .collect();
    assert_eq!(values.len(), native_ids(&db, "SELECT id FROM orders").len());
    assert!(values.iter().all(|value| (0.0..1.0).contains(value)));
}
