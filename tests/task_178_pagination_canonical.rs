//! Meaning-preserving TOP/FETCH/LIMIT SQL must yield the same *entire*
//! public typed protocol when no session, tie, or vendor law changes meaning.
//! Independent native equivalence remains a separate engine-specific gate.

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const DIALECTS: &[&str] = &[
    "ansi",
    "bigquery",
    "clickhouse",
    "databricks",
    "duckdb",
    "generic",
    "hive",
    "mssql",
    "mysql",
    "postgresql",
    "redshift",
    "snowflake",
    "sqlite",
];

fn protocol(sql: &str, dialect_name: &str) -> Value {
    let dialect = dialect_from_name(dialect_name).expect("13 exposed dialects");
    let protocol = analyze_sql(sql, dialect_name, dialect.as_ref()).expect("shared SQL syntax");
    assert!(
        protocol
            .statements()
            .iter()
            .all(|item| !matches!(item, ProtocolStatement::Unsupported(_))),
        "one of the two equivalent SQL variants was unsupported"
    );
    let mut value: Value = serde_json::from_str(&to_json(&protocol)).expect("public protocol");
    for input in value["inputs"].as_array_mut().expect("inputs") {
        input["dialect"] = Value::Null;
    }
    value
}

fn difference(left: &Value, right: &Value, path: &str) -> Option<String> {
    if left == right {
        return None;
    }
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
            for key in keys {
                if a.get(key) != b.get(key) {
                    let next = format!("{path}.{key}");
                    return match (a.get(key), b.get(key)) {
                        (Some(lhs), Some(rhs)) => difference(lhs, rhs, &next),
                        _ => Some(next),
                    };
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for index in 0..a.len().max(b.len()) {
                if a.get(index) != b.get(index) {
                    let next = format!("{path}[{index}]");
                    return match (a.get(index), b.get(index)) {
                        (Some(lhs), Some(rhs)) => difference(lhs, rhs, &next),
                        _ => Some(next),
                    };
                }
            }
        }
        _ => {}
    }
    Some(format!("{path}: {left} vs {right}"))
}

#[test]
fn top_fetch_and_limit_without_ties_have_the_same_full_semantic_document() {
    let reference_sql = "SELECT id FROM orders ORDER BY id LIMIT 2";
    for (variant, sql) in [
        ("TOP", "SELECT TOP 2 id FROM orders ORDER BY id"),
        (
            "FETCH",
            "SELECT id FROM orders ORDER BY id FETCH FIRST 2 ROWS ONLY",
        ),
    ] {
        for dialect in DIALECTS {
            let left = protocol(sql, dialect);
            let right = protocol(reference_sql, dialect);
            assert_eq!(
                left,
                right,
                "{variant}/{dialect}: equivalent row-selection SQL has divergent canonical facts at {}",
                difference(&left, &right, "$").unwrap_or_default()
            );
        }
    }
}
