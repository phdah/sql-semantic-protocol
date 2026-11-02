//! TASK-159: all exposed dialect parsers are classified without vendor execution claims.

use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput, WriteKind, WriteLifecycleAction,
};
use sqlparser::parser::Parser;

const DIALECTS: [&str; 13] = [
    "generic",
    "mysql",
    "postgresql",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];

#[test]
fn each_shared_lifecycle_ast_is_canonical_or_a_parser_boundary_in_all_dialects() {
    let forms = [
        ("TRUNCATE TABLE t", "truncate"),
        ("DROP TABLE t", "drop"),
        ("DROP TABLE IF EXISTS t", "drop"),
        ("ALTER TABLE t RENAME TO u", "rename"),
        ("RENAME TABLE t TO u", "rename"),
    ];
    let mut cells = 0usize;
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for (sql, kind) in forms {
            cells += 1;
            if Parser::parse_sql(parser.as_ref(), sql).is_err() {
                continue;
            }
            let analyzed =
                analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).unwrap();
            let statement = &analyzed.inputs()[0].statements()[0];
            let ProtocolStatement::Query(query) = statement else {
                panic!("{dialect}: parsed shared lifecycle without canonical action: {sql}");
            };
            let operation = query
                .write()
                .expect("lifecycle must retain target identity");
            assert_eq!(operation.kind(), WriteKind::Lifecycle, "{dialect}: {sql}");
            let actual = match operation.lifecycle_action().unwrap() {
                WriteLifecycleAction::Truncate => "truncate",
                WriteLifecycleAction::Drop { .. } => "drop",
                WriteLifecycleAction::Rename { .. } => "rename",
            };
            assert_eq!(actual, kind, "{dialect}: {sql}");
        }
    }
    assert_eq!(cells, 13 * forms.len());
}

#[test]
fn conditional_vendor_and_session_forms_fail_closed_when_the_ast_parses() {
    let conditional = [
        "DROP MATERIALIZED VIEW mv",
        "DROP TABLE t CASCADE",
        "TRUNCATE TABLE t RESTART IDENTITY",
        "CREATE TEMP TABLE t(a INTEGER)",
        "CREATE TABLE IF NOT EXISTS t(a INTEGER)",
        "CREATE TABLE copy LIKE t",
    ];
    let mut cells = 0usize;
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for sql in conditional {
            cells += 1;
            if Parser::parse_sql(parser.as_ref(), sql).is_err() {
                continue;
            }
            let analyzed =
                analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).unwrap();
            match &analyzed.inputs()[0].statements()[0] {
                ProtocolStatement::Unsupported(_) => {}
                ProtocolStatement::Query(query) => {
                    let write = query.write();
                    // Some dialects permit CREATE with vendor options; if recognized
                    // but not certified, its storage/session marker must remain true.
                    assert!(
                        write.is_some_and(|w| w.definition_execution_unknown()),
                        "{dialect} unproved vendor/session form became portable: {sql}"
                    );
                }
            }
        }
    }
    assert_eq!(cells, 13 * conditional.len());
}
