//! TASK-164 parser/canonical classification across the thirteen public dialects.
//! Parser acceptance is not an assertion of native transaction execution.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput, WriteKind,
};
use sqlparser::parser::Parser;

fn classify(dialect_name: &str, sql: &str) -> &'static str {
    let dialect = dialect_from_name(dialect_name).expect("registered public dialect");
    let parsed = match Parser::parse_sql(dialect.as_ref(), sql) {
        Ok(parsed) => parsed,
        Err(_) => return "parser_boundary",
    };
    if parsed.len() != 1 {
        return "compound_or_empty_boundary";
    }
    if matches!(
        sql,
        "BEGIN" | "COMMIT" | "ROLLBACK" | "SAVEPOINT x"
            | "ROLLBACK TO SAVEPOINT x" | "RELEASE SAVEPOINT x"
    ) {
        return "transaction_control_requires_native_session_evidence";
    }
    let analyzed = match analyze_inputs(
        &[SqlInput::inline(sql)],
        dialect_name,
        dialect.as_ref(),
    ) {
        Ok(bundle) => bundle,
        Err(_) => return "canonical_analysis_boundary",
    };
    match analyzed.inputs()[0].statements().first() {
        Some(ProtocolStatement::Query(query)) => match query.write().map(|write| write.kind()) {
            Some(WriteKind::Definition) => "definition",
            Some(WriteKind::Append) => "insert",
            Some(WriteKind::ConditionalMutation) => "merge",
            Some(WriteKind::Delete) => "delete",
            Some(WriteKind::Update) => "update",
            Some(WriteKind::SchemaAlter) => "alter",
            Some(WriteKind::Lifecycle) => "lifecycle",
            None => "read",
        },
        Some(ProtocolStatement::Unsupported(_)) | None => "canonical_unsupported",
        Some(_) => "canonical_unsupported",
    }
}

#[test]
fn task_164_all_thirteen_dialects_receive_explicit_parser_or_canonical_classifications() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-164-acceptance-freeze.json")).unwrap();
    let dialects = freeze["dialects"].as_array().unwrap();
    let cases = freeze["cases"].as_array().unwrap();
    let mut matrix = BTreeMap::new();
    let mut cells = 0;
    for dialect in dialects {
        let name = dialect.as_str().unwrap();
        let mut per_case = BTreeMap::new();
        for case in cases {
            let sql = case["program"].as_array().unwrap();
            let classes = sql
                .iter()
                .map(|item| classify(name, item.as_str().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(classes.len(), sql.len());
            assert!(
                classes.iter().all(|category| !category.is_empty()),
                "{} has a missing parser classification in {}",
                case["id"],
                name
            );
            cells += classes.len();
            per_case.insert(case["id"].as_str().unwrap().to_string(), classes);
        }
        matrix.insert(name.to_string(), per_case);
    }
    assert_eq!(dialects.len(), 13);
    assert_eq!(matrix.len(), 13);
    assert_eq!(cases.len(), 8);
    assert_eq!(
        cells,
        cases
            .iter()
            .map(|case| case["program"].as_array().unwrap().len())
            .sum::<usize>()
            * 13
    );

    // The native DuckDB path is the sole engine oracle; common canonical
    // forms must not silently turn into parser-only vendor execution claims.
    let duckdb = matrix.get("duckdb").unwrap();
    let core = &duckdb["84-C164-P-mixed"];
    assert_eq!(core[0], "definition");
    assert_eq!(core[1], "definition");
    assert_eq!(core[2], "insert");
    assert_eq!(core[3], "insert");
    assert_eq!(core[4], "merge");
    assert_eq!(core[5], "delete");
    assert_eq!(core[6], "read");
    let rollback = &duckdb["84-C164-P-rollback"];
    assert_eq!(rollback[4], "transaction_control_requires_native_session_evidence");
    assert_eq!(rollback[7], "transaction_control_requires_native_session_evidence");
    assert_eq!(freeze["native_engine"], "duckdb ~1.10506.0");
}

#[test]
fn parsed_vendor_transaction_syntax_does_not_change_frozen_residual_ownership() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-164-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["cases"][5]["owner"], "TASK-162");
    assert_eq!(freeze["cases"][5]["classification"], "Residual");
    assert_eq!(freeze["cases"][6]["owner"], "TASK-163");
    assert_eq!(freeze["cases"][6]["classification"], "Residual");
    assert_eq!(classify("duckdb", "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET v = s.v"), "merge");
}
