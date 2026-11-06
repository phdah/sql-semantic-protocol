//! TASK-163: parser acceptance across all exposed dialects is not
//! certification of engine or transaction session behavior.

use serde_json::{Map, Value};
use sql_semantic_protocol::dialect_from_name;
use sqlparser::{ast::Statement, parser::Parser};

const CASES: [(&str, &str); 8] = [
    ("begin", "BEGIN"),
    ("rollback", "ROLLBACK"),
    ("create_table", "CREATE TABLE t (id INTEGER, v INTEGER)"),
    ("drop_table", "DROP TABLE t"),
    ("temporary_table", "CREATE TEMP TABLE temp_t (id INTEGER)"),
    ("defer_constraints", "SET CONSTRAINTS ALL DEFERRED"),
    (
        "isolation_level",
        "BEGIN TRANSACTION ISOLATION LEVEL SERIALIZABLE",
    ),
    ("commit_chain", "COMMIT AND CHAIN"),
];

fn classify_parser(dialect: &str, sql: &str) -> &'static str {
    let dialect = dialect_from_name(dialect).expect("public dialect must resolve");
    let parsed = match Parser::parse_sql(dialect.as_ref(), sql) {
        Ok(parsed) => parsed,
        Err(_) => return "parser_boundary",
    };
    match parsed.as_slice() {
        [Statement::StartTransaction { .. }] => "start_transaction",
        [Statement::Commit { .. }] => "commit",
        [Statement::Rollback { .. }] => "rollback",
        [Statement::CreateTable(_)] => "create_table",
        [Statement::Drop { .. }] => "drop",
        [_] => "parsed_other_ast",
        _ => "not_single_statement",
    }
}

#[test]
fn all_frozen_task_163_parser_cells_match_explicit_expected_boundaries() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-163-acceptance-freeze.json")).unwrap();
    let dialects = freeze["dialects"].as_array().expect("13 public dialects");
    let mut observed = Map::new();
    for dialect in dialects {
        let name = dialect.as_str().unwrap();
        let cases = CASES
            .iter()
            .map(|(id, sql)| {
                (
                    (*id).to_string(),
                    Value::String(classify_parser(name, sql).to_string()),
                )
            })
            .collect::<Map<_, _>>();
        observed.insert(name.to_string(), Value::Object(cases));
    }
    assert_eq!(dialects.len(), 13);
    assert_eq!(observed.len() * CASES.len(), 104);
    assert_eq!(
        Value::Object(observed.clone()),
        freeze["dialect_parser_matrix_expected"],
        "TASK163_MATRIX_OBSERVED={}",
        Value::Object(observed)
    );
    assert_eq!(freeze["semantics"]["native_engine"], "duckdb ~1.10506.0");
}
