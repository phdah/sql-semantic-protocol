//! Semantics-preserving SQL spellings, with a negative-control registry.
//!
//! The public protocol is compared in full, removing only input dialect
//! provenance, and both SQL spellings must execute against the same complete
//! physical DuckDB bag. Parser acceptance alone is never certification.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const FIXTURE: &str = include_str!("../docs/task-178-acceptance-freeze.json");
const PAIRS: &str = include_str!("../docs/task-178-syntax-pairs.json");

fn parse(value: &str) -> Value {
    serde_json::from_str(value).expect("task-owned fixture")
}

fn original_sql<'a>(case: &'a Value, side: &str) -> &'a str {
    case[side].as_str().expect("fixed SQL spelling")
}

fn document(sql: &str, dialect_name: &str) -> Value {
    let dialect = dialect_from_name(dialect_name).expect("named dialect");
    let protocol = analyze_sql(sql, dialect_name, dialect.as_ref())
        .unwrap_or_else(|e| panic!("SQL dialect {dialect_name}: {e}"));
    assert!(
        protocol
            .statements()
            .iter()
            .all(|statement| !matches!(statement, ProtocolStatement::Unsupported(_))),
        "unsupported SQL cannot be relabeled as equivalent: {dialect_name}/{sql}"
    );
    let mut value = parse(&to_json(&protocol));
    for input in value["inputs"].as_array_mut().expect("inputs") {
        input["dialect"] = Value::Null;
    }
    value
}

fn rows(db: &Connection, sql: &str) -> Vec<Vec<Option<i64>>> {
    let mut statement = db.prepare(sql).expect("native SQL syntax");
    let result = statement
        .query_map([], |row| row.get::<_, Option<i64>>(0).map(|val| vec![val]))
        .expect("native output");
    let mut bag: Vec<_> = result.map(|row| row.expect("physical row")).collect();
    bag.sort();
    bag
}

fn wanted(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut bag: Vec<Vec<Option<i64>>> =
        serde_json::from_value(value.clone()).expect("explicit nullable SQL bag");
    bag.sort();
    bag
}

#[test]
fn portable_spelling_pairs_have_identical_complete_protocol_for_thirteen_dialects() {
    let fixture = parse(PAIRS);
    let cases = fixture["exact_synonym_cases"]
        .as_array()
        .expect("exact pairs");
    assert_eq!(fixture["schema_version"], 1);
    assert_eq!(cases.len(), 3);
    assert_eq!(fixture["dialects"].as_array().expect("dialects").len(), 13);
    for pair in cases {
        let id = pair["id"].as_str().expect("stable case");
        let left = original_sql(pair, "left");
        let right = original_sql(pair, "right");
        for dialect in fixture["dialects"].as_array().expect("13 dialects") {
            let name = dialect.as_str().expect("dialect label");
            assert_eq!(
                document(left, name),
                document(right, name),
                "{id}/{name}: equivalent SQL spellings must have the same *entire* public semantic document"
            );
        }
    }
}

#[test]
fn portable_spelling_pairs_have_independent_native_duckdb_null_duplicate_bags() {
    let fixture = parse(PAIRS);
    let source = parse(FIXTURE);
    for pair in fixture["exact_synonym_cases"]
        .as_array()
        .expect("exact pairs")
    {
        let db = Connection::open_in_memory().expect("in-memory DuckDB");
        db.execute_batch(source["source_setup"].as_str().expect("complete input"))
            .expect("complete NULL and duplicate source");
        for name in ["orders", "customers"] {
            let mut source_bag: Vec<Vec<Option<i64>>> = {
                let mut stmt = db
                    .prepare(&format!("SELECT id, amount FROM {name}"))
                    .expect("source bag");
                stmt.query_map([], |row| {
                    Ok(vec![
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                    ])
                })
                .expect("full source result")
                .map(|row| row.expect("physical source row"))
                .collect()
            };
            source_bag.sort();
            assert_eq!(
                source_bag,
                wanted(&source["source_bags"][name]),
                "{name}: full physical input bag"
            );
        }
        let expected = wanted(&pair["expected_terminal_bag"]);
        for side in ["left", "right"] {
            assert_eq!(
                rows(&db, original_sql(pair, side)),
                expected,
                "{}: {side} must match *independent* full native output bag",
                pair["id"]
            );
        }
    }
}

#[test]
fn distinct_dialect_and_materialization_laws_remain_named_unapproved_boundaries() {
    let j = parse(PAIRS);
    let boundaries = j["explicit_noncertified_or_non_equivalent_cases"]
        .as_array()
        .expect("conditional and negative controls");
    for label in ["TOP", "FETCH", "MINUS", "WINDOW", "INSERT", "DDL"] {
        let id = format!("88-C178-SYN-{label}");
        assert!(
            boundaries
                .iter()
                .any(|boundary| boundary["id"] == id
                    && boundary["required_evidence"].as_str().is_some()),
            "{id}: named variant must not be silently omitted"
        );
    }
    assert_eq!(j["release_approved"], false);
    assert_eq!(j["independent_signoff"], false);
}
