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
    let mut observed = std::collections::BTreeMap::new();
    for dialect in f["dialects"].as_array().unwrap() {
        let name = dialect.as_str().unwrap();
        let parser = dialect_from_name(name).unwrap();
        let mut results = Vec::new();
        for case in f["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            let sql = case["sql"][0].as_str().unwrap();
            checked += 1;
            if Parser::parse_sql(parser.as_ref(), sql).is_err() {
                boundaries += 1;
                results.push("parser_boundary");
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
                results.push("canonical_unsupported");
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
            results.push(match mode {
                WriteDefinitionMode::CreateTable => "create_table",
                WriteDefinitionMode::ReplaceTable => "replace_table",
                WriteDefinitionMode::CreateView => "create_view",
                WriteDefinitionMode::ReplaceView => "replace_view",
                WriteDefinitionMode::CreateMaterializedView => "create_materialized_view",
                WriteDefinitionMode::ReplaceMaterializedView => "replace_materialized_view",
            });
            if mode == WriteDefinitionMode::CreateMaterializedView {
                assert!(
                    !write.view_execution_unknown(),
                    "{name}/{id}: plain snapshot definition"
                );
            }
        }
        observed.insert(name.to_owned(), results);
    }
    assert_eq!(
        serde_json::to_value(&observed).unwrap(),
        f["dialect_expected"],
        "every dialect and fixture requires a frozen parser/canonical outcome"
    );
    assert_eq!(checked, 117);
    assert_eq!(checked, parsed + boundaries);
    assert!(parsed > 0);
    assert_eq!(f["coverage"]["dialect_cells_required"], 117);
}

#[test]
fn session_and_vendor_view_options_cannot_erase_execution_uncertainty() {
    for (dialect, sql) in [
        ("duckdb", "CREATE TEMP VIEW view_stage AS SELECT k FROM raw"),
        (
            "postgresql",
            "CREATE VIEW view_stage WITH (security_barrier = true) AS SELECT k FROM raw",
        ),
        (
            "clickhouse",
            "CREATE MATERIALIZED VIEW snap_stage TO sink AS SELECT k FROM raw",
        ),
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

#[test]
fn portable_ctas_does_not_upgrade_unproved_temporary_storage_laws() {
    use sql_semantic_protocol::{
        physical_ordered_write_state_plan, to_bundle_json, PhysicalWriteStatePlan,
    };
    let dialect = dialect_from_name("snowflake").unwrap();
    let sql = "CREATE TEMPORARY TABLE temp_stage AS SELECT k FROM raw";
    let bundle = analyze_inputs(&[SqlInput::inline(sql)], "snowflake", dialect.as_ref()).unwrap();
    let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
        panic!("temporary CTAS must retain its canonical query identity");
    };
    assert_eq!(
        query.write().unwrap().definition_mode(),
        Some(WriteDefinitionMode::CreateTable)
    );
    assert!(query.write().unwrap().definition_execution_unknown());
    let wire: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    assert_eq!(
        wire["inputs"][0]["statements"][0]["write"]["definition_execution_unknown"],
        true
    );
    let plan = physical_ordered_write_state_plan(&bundle, &[]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn parsed_table_storage_options_never_claim_plain_create_exactness() {
    let examples = [
        (
            "sqlite",
            "CREATE TABLE storage_stage (k INTEGER) STRICT",
            false,
        ),
        (
            "sqlite",
            "CREATE TABLE storage_stage (k INTEGER) WITHOUT ROWID",
            false,
        ),
        (
            "postgresql",
            "CREATE TABLE storage_stage (k INTEGER) WITH (fillfactor = 70)",
            false,
        ),
        (
            "postgresql",
            "CREATE TABLE storage_stage WITH (fillfactor = 70) AS SELECT k FROM raw",
            true,
        ),
    ];
    let mut parsed = 0;
    for (dialect, sql, has_query) in examples {
        let parser = dialect_from_name(dialect).unwrap();
        if Parser::parse_sql(parser.as_ref(), sql).is_err() {
            continue;
        }
        parsed += 1;
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()).unwrap();
        match &bundle.inputs()[0].statements()[0] {
            ProtocolStatement::Unsupported(_) if !has_query => {}
            ProtocolStatement::Query(query) if has_query => {
                assert!(
                    query.write().unwrap().definition_execution_unknown(),
                    "{dialect}: parser-accepted storage variant cannot be an exact CREATE"
                );
            }
            statement => {
                panic!("{dialect}: unsupported storage variant must fail closed: {statement:?}")
            }
        }
    }
    assert!(
        parsed >= 2,
        "at least two storage variants must be exercised"
    );
}

#[test]
fn materialized_view_replace_is_not_inferred_from_plain_snapshot_support() {
    let sql = "CREATE OR REPLACE MATERIALIZED VIEW snap_stage AS SELECT k FROM raw";
    let mut parsed = 0;
    for dialect in freeze()["dialects"].as_array().unwrap() {
        let name = dialect.as_str().unwrap();
        let parser = dialect_from_name(name).unwrap();
        if Parser::parse_sql(parser.as_ref(), sql).is_err() {
            continue;
        }
        parsed += 1;
        let bundle = analyze_inputs(&[SqlInput::inline(sql)], name, parser.as_ref()).unwrap();
        let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
            panic!("{name}: materialized replacement must keep its canonical operation");
        };
        let write = query.write().unwrap();
        assert_eq!(
            write.definition_mode(),
            Some(WriteDefinitionMode::ReplaceMaterializedView)
        );
        assert!(
            write.view_execution_unknown(),
            "{name}: native replacement/session semantics cannot be inferred"
        );
    }
    assert!(
        parsed > 0,
        "at least one parser must accept this vendor boundary"
    );
}
