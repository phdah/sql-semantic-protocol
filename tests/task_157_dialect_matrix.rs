//! TASK-157 per-variant parser and canonical DDL identity classification.
//! A parser boundary is recorded independently of engine execution support.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput, WriteDefinitionMode,
};
use sqlparser::parser::Parser;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-157-acceptance-freeze.json")).unwrap()
}

#[test]
fn all_thirteen_dialects_classify_each_frozen_definition_without_invented_physical_proofs() {
    let f = freeze();
    let mut checked = 0;
    let mut parsed = 0;
    let mut boundaries = 0;
    for dialect in f["dialects"].as_array().unwrap() {
        let name = dialect.as_str().unwrap();
        let parser = dialect_from_name(name).unwrap();
        for case in f["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            let sql = case["sql"][0].as_str().unwrap();
            checked += 1;
            if Parser::parse_sql(parser.as_ref(), sql).is_err() {
                boundaries += 1;
                assert!(
                    analyze_inputs(&[SqlInput::inline(sql)], name, parser.as_ref()).is_err(),
                    "{name}/{id}: rejected parser syntax must not normalize"
                );
                continue;
            }
            parsed += 1;
            let bundle = analyze_inputs(&[SqlInput::inline(sql)], name, parser.as_ref())
                .unwrap_or_else(|error| panic!("{name}/{id}: {error}"));
            let statement = &bundle.inputs()[0].statements()[0];
            if case["classification"] == "Residual" {
                assert!(
                    matches!(statement, ProtocolStatement::Unsupported(_)),
                    "{name}/{id}: conditional existence must be explicitly unsupported"
                );
                continue;
            }
            let ProtocolStatement::Query(query) = statement else {
                panic!("{name}/{id}: supported DDL must have canonical query identity");
            };
            assert_eq!(query.produced_relation(), case["target"].as_str());
            let write = query.write().unwrap();
            let mode = match id {
                "83-C157-P-replace" => WriteDefinitionMode::ReplaceTable,
                "83-C157-P-logical-view" => WriteDefinitionMode::CreateView,
                "83-C157-P-materialized" => WriteDefinitionMode::CreateMaterializedView,
                _ => WriteDefinitionMode::CreateTable,
            };
            assert_eq!(write.definition_mode(), Some(mode), "{name}/{id}");
            if mode == WriteDefinitionMode::CreateMaterializedView {
                assert!(!write.view_execution_unknown(), "{name}/{id}: plain snapshot definition");
            }
        }
    }
    assert_eq!(checked, 117);
    assert_eq!(checked, parsed + boundaries);
    assert!(parsed > 0);
    assert_eq!(f["coverage"]["dialect_cells_required"], 117);
}

#[test]
fn session_and_vendor_view_options_cannot_erase_execution_uncertainty() {
    for (dialect, sql) in [
        ("duckdb", "CREATE TEMP VIEW view_stage AS SELECT k FROM raw"),
        ("postgresql", "CREATE VIEW view_stage WITH (security_barrier = true) AS SELECT k FROM raw"),
        ("clickhouse", "CREATE MATERIALIZED VIEW snap_stage TO sink AS SELECT k FROM raw"),
    ] {
        let parser = dialect_from_name(dialect).unwrap();
        if Parser::parse_sql(parser.as_ref(), sql).is_err() {
            continue;
        }
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).unwrap();
        let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
            panic!("{dialect}: parser-accepted view must remain canonical");
        };
        assert!(query.write().unwrap().view_execution_unknown());
    }
}
