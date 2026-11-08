//! TASK-177 first-party complete relational bag oracles for original TASK-66 SQL.
//!
//! Expected bags are literal, independent fixtures. DuckDB executes the exact
//! registered SQL over full NULL/duplicate physical sources. These checks do
//! not certify strongest typed domains, 13 vendor runtimes, or source adapters.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, ProtocolStatement};

const FIXTURE: &str = include_str!("../docs/task-177-relational-bag-oracles.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned native oracle fixture")
}

fn expected_bag(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut result: Vec<_> = value
        .as_array()
        .expect("literal complete physical bag")
        .iter()
        .map(|row| {
            row.as_array()
                .expect("physical row")
                .iter()
                .map(|item| {
                    if item.is_null() {
                        None
                    } else {
                        Some(item.as_i64().expect("integer"))
                    }
                })
                .collect()
        })
        .collect();
    result.sort();
    result
}

fn actual_bag(db: &Connection, sql: &str, columns: usize) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("original native SQL");
    assert_eq!(stmt.column_count(), columns, "native arity for {sql}");
    let rows = stmt
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get::<usize, Option<i64>>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("native output rows");
    let mut result: Vec<_> = rows.map(|row| row.expect("typed physical value")).collect();
    result.sort();
    result
}

fn verify_group(group: &str) {
    let fixture = parse(FIXTURE);
    let samples = parse(SAMPLES);
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(fixture["owner"], "TASK-177");
    assert_eq!(fixture["case_count"], 51);
    let groups = fixture["groups"]
        .as_object()
        .expect("three finite families");
    assert_eq!(groups.len(), 3);
    let counted: usize = groups
        .values()
        .map(|ids| ids.as_array().expect("stable sample IDs").len())
        .sum();
    assert_eq!(counted, 51, "a named native oracle was lost");

    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(
        fixture["source_setup"]
            .as_str()
            .expect("physical source DDL and rows"),
    )
    .expect("complete native physical input");
    for (relation, columns) in [("orders", 2), ("customers", 2), ("products", 1)] {
        assert_eq!(
            actual_bag(&db, &format!("SELECT * FROM {relation}"), columns),
            expected_bag(&fixture["source_bags"][relation]),
            "{relation}: complete input bag (including duplicate occurrences)"
        );
    }

    let dialect = dialect_from_name("duckdb").expect("named parser dialect");
    let cases = fixture["cases"].as_array().expect("fixed oracles");
    let ids = fixture["groups"][group]
        .as_array()
        .expect("named native family");
    for id in ids {
        let id = id.as_str().expect("stable case ID");
        let case = cases
            .iter()
            .find(|candidate| candidate["id"] == id)
            .expect("every group case has pinned physical proof");
        let sample = samples["cases"]
            .as_array()
            .expect("TASK-66 sample registry")
            .iter()
            .find(|sample| sample["id"] == id)
            .expect("source case exists");
        assert_eq!(
            case["sql"], sample["sql"],
            "{id}: original SQL may not drift"
        );
        assert_eq!(
            case["feature_id"], sample["feature_id"],
            "{id}: feature identity"
        );
        assert_eq!(case["variant"], sample["variant"], "{id}: variant identity");
        let sql = case["sql"].as_str().expect("original SQL");
        let columns = case["columns"].as_u64().expect("projection arity") as usize;
        assert_eq!(
            actual_bag(&db, sql, columns),
            expected_bag(&case["expected_terminal_bag"]),
            "{id}: complete independently pinned native terminal bag"
        );

        let protocol = analyze_sql(sql, "duckdb", dialect.as_ref())
            .unwrap_or_else(|error| panic!("{id}: public API must analyze: {error}"));
        let [ProtocolStatement::Query(query)] = protocol.statements() else {
            panic!("{id}: query must have explicit canonical query semantics");
        };
        assert_eq!(
            query.output().columns().len(),
            columns,
            "{id}: native projection arity and canonical output must agree"
        );
    }
}

#[test]
fn native_joins_preserve_identity_and_duplicate_multiplicity() {
    verify_group("joins");
}

#[test]
fn native_set_operators_preserve_null_and_bag_multiplicity() {
    verify_group("sets");
}

#[test]
fn native_grouped_outputs_preserve_null_and_aggregate_multiplicity() {
    verify_group("grouping");
}
