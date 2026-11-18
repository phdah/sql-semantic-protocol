//! TASK-191 independent replay of the 23 original nonterminal SQL fixtures.
//!
//! A successful companion, observed stochastic output, or error is not a
//! successful fixed terminal bag for the original SQL statement.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::Connection;
use serde_json::Value;

const REVIEW: &str = include_str!("../docs/task-191-reviewed-dispositions-v2.json");
const ORIGINALS: &str = include_str!("../docs/task-177-parser-samples.json");
const SOURCES: &str = include_str!("../docs/task-177-relational-bag-oracles.json");
const BOUNDARIES: &str = include_str!("../docs/task-177-native-boundary-probes.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("immutable task-owned native oracle")
}

fn entries<'a>(evidence: &'a Value, key: &str) -> &'a [Value] {
    evidence[key].as_array().expect("bounded evidence list")
}

fn original<'a>(samples: &'a Value, id: &str) -> &'a str {
    entries(samples, "cases")
        .iter()
        .find(|candidate| candidate["id"] == id)
        .expect("frozen original SQL")["sql"]
        .as_str()
        .expect("literal original SQL")
}

fn fresh_source() -> Connection {
    let db = Connection::open_in_memory().expect("pinned DuckDB");
    let sources = read(SOURCES);
    db.execute_batch(
        sources["source_setup"]
            .as_str()
            .expect("fixed DDL and inserts"),
    )
    .expect("complete source population");
    db
}

type SourceRow = (i64, Vec<Option<i64>>);
type SourceBag = BTreeMap<String, Vec<SourceRow>>;

fn all_source_rows(db: &Connection) -> SourceBag {
    let sources = read(SOURCES);
    let mut result = BTreeMap::new();
    for (name, width) in [("orders", 2usize), ("customers", 2), ("products", 1)] {
        let mut stmt = db
            .prepare(&format!("SELECT rowid, * FROM {name} ORDER BY rowid"))
            .expect("physical rowid and all source columns");
        let rows: Vec<SourceRow> = stmt
            .query_map([], |row| {
                let identity: i64 = row.get(0)?;
                let values: Vec<Option<i64>> = (0..width)
                    .map(|index| row.get(index + 1))
                    .collect::<duckdb::Result<_>>()?;
                Ok((identity, values))
            })
            .expect("complete physical bag")
            .map(|r| r.expect("source row"))
            .collect();
        let mut actual: Vec<_> = rows.iter().map(|(_, values)| values.clone()).collect();
        actual.sort();
        let mut expected: Vec<Vec<Option<i64>>> =
            serde_json::from_value(sources["source_bags"][name].clone())
                .expect("independent complete source bag");
        expected.sort();
        assert_eq!(
            actual, expected,
            "{name}: complete source bag must match fixture"
        );
        let unique: BTreeSet<i64> = rows.iter().map(|(identity, _)| *identity).collect();
        assert_eq!(
            unique.len(),
            rows.len(),
            "{name}: stable physical row instance"
        );
        result.insert(name.to_string(), rows);
    }
    result
}

fn exhaust_original(db: &Connection, sql: &str) -> duckdb::Result<()> {
    if sql.trim_start().starts_with("SELECT") {
        db.prepare(sql).and_then(|mut stmt| {
            let mut rows = stmt.query([])?;
            while rows.next()?.is_some() {}
            Ok(())
        })
    } else {
        db.execute_batch(sql)
    }
}

fn source_ids(db: &Connection) -> Vec<Option<i64>> {
    let mut stmt = db
        .prepare("SELECT id FROM orders")
        .expect("original source");
    stmt.query_map([], |row| row.get::<_, Option<i64>>(0))
        .expect("integer source column")
        .map(|r| r.expect("source identity value"))
        .collect()
}

fn counts(values: &[Option<i64>]) -> BTreeMap<Option<i64>, usize> {
    let mut counted = BTreeMap::new();
    for value in values {
        *counted.entry(*value).or_insert(0) += 1;
    }
    counted
}

#[test]
fn all_15_original_duckdb_error_cases_fail_without_any_mutation_or_output_bag() {
    let review = read(REVIEW);
    let originals = read(ORIGINALS);
    let cells = entries(&review, "native_original_cases");
    let negative: Vec<_> = cells
        .iter()
        .filter(|r| r["proof_obligation"] == "explicit_native_rejection_or_conditional_syntax")
        .collect();
    assert_eq!(negative.len(), 15);
    for evidence in negative {
        let id = evidence["original_id"].as_str().expect("original identity");
        let sql = original(&originals, id);
        assert_eq!(evidence["original_sql"], sql);
        let db = fresh_source();
        let before = all_source_rows(&db);
        let error = exhaust_original(&db, sql)
            .expect_err("pinned native original must reject/error, not generate a terminal");
        assert!(
            !error.to_string().is_empty(),
            "{id}: original error must have an inspectable native diagnostic"
        );
        assert_eq!(
            all_source_rows(&db),
            before,
            "{id}: failed original SQL must not change any physical row instance"
        );
        assert_eq!(evidence["original_fixed_terminal_certified"], false);
        assert_eq!(
            evidence["reviewed_classification"],
            "Impossible_successful_original_terminal_in_pinned_DuckDB"
        );
    }
}

#[test]
fn sampling_five_original_cases_preserves_full_input_and_never_invents_rows() {
    let review = read(REVIEW);
    let originals = read(ORIGINALS);
    let rows: Vec<_> = entries(&review, "native_original_cases")
        .iter()
        .filter(|r| r["proof_obligation"] == "engine_sample_nondeterminism")
        .collect();
    assert_eq!(rows.len(), 5);
    let db = fresh_source();
    let before = all_source_rows(&db);
    let source = source_ids(&db);
    let source_counts = counts(&source);
    assert_eq!(source.len(), 6);
    for evidence in rows {
        let id = evidence["original_id"].as_str().expect("original ID");
        let sql = original(&originals, id);
        // DuckDB may require the explicit PERCENT synonym. The alternative
        // is a separate control and never certifies the original spelling.
        let executable = match db.prepare(sql) {
            Ok(_) => sql.to_string(),
            Err(native_error) => {
                assert!(
                    native_error.to_string().to_lowercase().contains("sample"),
                    "{id}: unexpected native failure: {native_error}"
                );
                assert!(sql.contains("(10)"));
                sql.replace("(10)", "(10 PERCENT)")
            }
        };
        let mut stmt = db.prepare(&executable).expect("bounded sample control");
        let actual: Vec<Option<i64>> = stmt
            .query_map([], |row| row.get(0))
            .expect("native sampled rows")
            .map(|row| row.expect("sample value"))
            .collect();
        for (value, multiplicity) in counts(&actual) {
            assert!(
                multiplicity <= source_counts.get(&value).copied().unwrap_or(0),
                "{id}: no source-id multiplicity may be amplified"
            );
        }
        assert_eq!(evidence["original_fixed_terminal_certified"], false);
    }
    assert_eq!(all_source_rows(&db), before);
}

#[test]
fn infinite_recursion_has_only_a_bounded_companion_oracle() {
    let review = read(REVIEW);
    let originals = read(ORIGINALS);
    let boundaries = read(BOUNDARIES);
    let record = entries(&review, "native_original_cases")
        .iter()
        .find(|r| r["proof_obligation"] == "recursive_nontermination")
        .expect("one unbounded recursive original");
    let sql = original(
        &originals,
        record["original_id"].as_str().expect("original ID"),
    );
    assert!(sql.contains("WITH RECURSIVE"));
    assert!(!sql.contains("WHERE n <"), "original must remain unbounded");
    let db = fresh_source();
    let before = all_source_rows(&db);
    let control = boundaries["bounded_companion"]
        .as_str()
        .expect("explicit terminating SQL");
    let mut stmt = db.prepare(control).expect("bounded alternative");
    let rows: Vec<Option<i64>> = stmt
        .query_map([], |r| r.get(0))
        .expect("bounded terminal")
        .map(|r| r.expect("output row"))
        .collect();
    assert_eq!(rows, vec![Some(1), Some(2), Some(3)]);
    assert_eq!(all_source_rows(&db), before);
    assert_eq!(record["original_fixed_terminal_certified"], false);
}

#[test]
fn random_and_explain_originals_keep_runtime_and_plan_uncertainty_explicit() {
    let review = read(REVIEW);
    let originals = read(ORIGINALS);
    let db = fresh_source();
    let before = all_source_rows(&db);
    let cases = entries(&review, "native_original_cases");

    let random = cases
        .iter()
        .find(|r| r["proof_obligation"] == "runtime_nondeterminism")
        .expect("one original random case");
    let random_sql = original(
        &originals,
        random["original_id"].as_str().expect("random ID"),
    );
    let mut random_statement = db.prepare(random_sql).expect("native RANDOM SQL");
    let values: Vec<f64> = random_statement
        .query_map([], |row| row.get(0))
        .expect("RANDOM physical output")
        .map(|value| value.expect("floating sample"))
        .collect();
    assert_eq!(values.len(), source_ids(&db).len());
    assert!(values.iter().all(|value| (0.0..1.0).contains(value)));
    assert_eq!(random["original_fixed_terminal_certified"], false);

    let explain = cases
        .iter()
        .find(|r| r["proof_obligation"] == "native_execution_candidate")
        .expect("one optimizer-sensitive EXPLAIN");
    let plan_sql = original(
        &originals,
        explain["original_id"].as_str().expect("plan ID"),
    );
    let mut stmt = db.prepare(plan_sql).expect("native EXPLAIN SQL");
    let plan_rows: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("plan metadata rows")
        .map(|value| value.expect("plan label and content"))
        .collect();
    assert!(
        plan_rows
            .iter()
            .any(|(label, content)| !label.is_empty() && !content.trim().is_empty()),
        "EXPLAIN must produce nonempty plan content in pinned DuckDB"
    );
    assert_eq!(explain["original_fixed_terminal_certified"], false);
    assert_eq!(all_source_rows(&db), before);
}
