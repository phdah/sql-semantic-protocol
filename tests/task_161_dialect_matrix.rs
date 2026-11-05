//! TASK-161: every frozen SQL statement has a pinned parser/canonical
//! classification in all thirteen exposed dialects. Parser acceptance alone
//! is never proof of native transaction or physical execution semantics.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput, WriteKind,
};
use sqlparser::parser::Parser;

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-161-acceptance-freeze.json")).unwrap()
}

fn classification(dialect: &str, sql: &str) -> String {
    let engine = dialect_from_name(dialect).unwrap();
    if Parser::parse_sql(engine.as_ref(), sql).is_err() {
        return "parser_boundary".to_string();
    }
    let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], dialect, engine.as_ref()) else {
        return "canonical_analysis_boundary".to_string();
    };
    match bundle.inputs()[0].statements().first() {
        Some(ProtocolStatement::Unsupported(_)) | None => "canonical_unsupported".to_string(),
        Some(ProtocolStatement::Query(query)) => match query.write().map(|w| w.kind()) {
            Some(WriteKind::Definition) => "definition",
            Some(WriteKind::Append) => "insert",
            Some(WriteKind::Update) => "update",
            Some(WriteKind::Delete) => "delete",
            Some(WriteKind::ConditionalMutation) => "merge",
            Some(WriteKind::SchemaAlter) => "alter",
            Some(WriteKind::Lifecycle) => "lifecycle",
            None => "read",
        }
        .to_string(),
        // ProtocolStatement is non-exhaustive to external consumers.
        Some(_) => "canonical_unsupported".to_string(),
    }
}

#[test]
fn frozen_case_by_dialect_parser_matrix_is_exact_and_does_not_infer_engine_laws() {
    let freeze = frozen();
    let dialects = freeze["dialects"].as_array().unwrap();
    let cases = freeze["cases"].as_array().unwrap();
    let mut observed = BTreeMap::new();
    let mut statement_cells = 0;
    for dialect in dialects {
        let dialect = dialect.as_str().unwrap();
        let mut variants = BTreeMap::new();
        for case in cases {
            let case_id = case["id"].as_str().unwrap();
            let statements = case["program"]
                .as_array()
                .unwrap()
                .iter()
                .map(|sql| classification(dialect, sql.as_str().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(statements.len(), case["program"].as_array().unwrap().len());
            statement_cells += statements.len();
            variants.insert(case_id.to_string(), statements);
        }
        observed.insert(dialect.to_string(), variants);
    }
    assert_eq!(dialects.len(), 13);
    assert_eq!(cases.len(), 7);
    assert_eq!(freeze["parser_dialect_cells"], 91);
    assert_eq!(
        statement_cells,
        freeze["parser_statement_cells"].as_u64().unwrap() as usize
    );
    let observed = serde_json::to_value(observed).unwrap();
    if freeze["case_dialect_expected"].is_null() {
        // Initial observation must be manually frozen, never silently treated
        // as approval of an unsupported dialect or an engine execution law.
        panic!("TASK161_DIALECT_OBSERVED={observed}");
    }
    assert_eq!(observed, freeze["case_dialect_expected"]);
    // Native execution certification belongs to DuckDB oracles. Vendor
    // parser-only acceptance cannot promote a conditional proof to Exact.
    assert!(freeze["conditional_engine_owner"] == "TASK-162");
}
