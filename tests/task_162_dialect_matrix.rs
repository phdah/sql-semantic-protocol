//! TASK-162: pin all 13 public sqlparser dialect boundaries separately
//! from native DuckDB engine certification.

use serde_json::Value;
use sql_semantic_protocol::dialect_from_name;
use sqlparser::{ast::Statement, parser::Parser};

const CASES: [(&str, &str); 7] = [
    ("begin", "BEGIN TRANSACTION"),
    ("commit", "COMMIT"),
    ("rollback", "ROLLBACK"),
    ("savepoint", "SAVEPOINT checkpoint"),
    ("rollback_to", "ROLLBACK TO SAVEPOINT checkpoint"),
    ("release", "RELEASE SAVEPOINT checkpoint"),
    ("commit_chain", "COMMIT AND CHAIN"),
];

fn parser_boundary(dialect: &str, sql: &str) -> String {
    let engine = dialect_from_name(dialect).unwrap();
    let parsed = match Parser::parse_sql(engine.as_ref(), sql) {
        Ok(parsed) => parsed,
        Err(_) => return "parser_boundary".to_string(),
    };
    match parsed.as_slice() {
        [Statement::StartTransaction { .. }] => "start_transaction",
        [Statement::Commit { .. }] => "commit",
        [Statement::Rollback { savepoint: None, .. }] => "rollback",
        [Statement::Rollback { savepoint: Some(_), .. }] => "rollback_to",
        [Statement::Savepoint { .. }] => "savepoint",
        [Statement::ReleaseSavepoint { .. }] => "release",
        [_] => "other_ast",
        _ => "multiple_statements",
    }.to_string()
}

#[test]
fn transaction_parser_boundary_matrix_is_explicitly_frozen_for_every_dialect() {
    let freeze: Value = serde_json::from_str(include_str!("../docs/task-162-acceptance-freeze.json")).unwrap();
    let dialects = freeze["dialects"].as_array().unwrap();
    let mut observed = serde_json::Map::new();
    for dialect in dialects {
        let dialect = dialect.as_str().unwrap();
        let cases = CASES.iter().map(|(name, sql)| {
            (name.to_string(), Value::String(parser_boundary(dialect, sql)))
        }).collect::<serde_json::Map<_, _>>();
        observed.insert(dialect.to_string(), Value::Object(cases));
    }
    assert_eq!(dialects.len(), 13);
    assert_eq!(observed.len() * CASES.len(), 91);
    assert_eq!(
        Value::Object(observed.clone()),
        freeze["dialect_parser_matrix_expected"],
        "TASK162_MATRIX_OBSERVED={}",
        Value::Object(observed)
    );
    // sqlparser support must never turn a vendor transaction into a
    // certified native DuckDB execution law.
    assert_eq!(freeze["native_engine_only"], "duckdb");
}
