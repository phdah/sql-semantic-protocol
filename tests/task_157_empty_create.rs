//! TASK-157: CREATE TABLE's empty physical relation is a real ordered DDL
//! transition, distinct from CTAS and an unmaterialized logical view.
//! The public protocol/write-state APIs are the verification seam.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, physical_ordered_write_state_plan, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, SqlInput,
};
use sqlparser::dialect::GenericDialect;
use std::collections::BTreeMap;

fn analyze(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    analyze_inputs(&[SqlInput::inline(sql)], "generic", &GenericDialect {}).unwrap()
}

fn row(value: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([(
        "k".to_string(),
        value.map_or(LiteralValue::Null, |value| {
            LiteralValue::Number(value.to_string())
        }),
    )]))
    .unwrap()
}

#[test]
fn create_table_with_declared_schema_constructs_an_empty_relation() {
    let sql = "CREATE TABLE empty_stage (k INTEGER)";
    let bundle = analyze(sql);
    assert_eq!(bundle.layers().len(), 1, "empty DDL must be a producer");
    let result = physical_ordered_write_state_plan(&bundle, &[]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = result
    else {
        panic!("empty CREATE TABLE must construct an exact state: {result:?}");
    };
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].after().relation(), "empty_stage");
    assert!(transitions[0].after().rows().is_empty());
    assert_eq!(final_states.len(), 1);
    assert_eq!(final_states[0].relation(), "empty_stage");
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == "empty_stage")
        .unwrap();
    let domains = final_states[0].exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].column(), "k");
    assert!(domains[0].values().is_empty());

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(sql).unwrap();
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM empty_stage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn create_table_rejects_an_existing_target_without_destroying_it() {
    let bundle = analyze("CREATE TABLE empty_stage (k INTEGER)");
    let original = PhysicalWriteState::new("empty_stage", vec![row(Some(7))]).unwrap();
    let plan = physical_ordered_write_state_plan(&bundle, std::slice::from_ref(&original));
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "{plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE empty_stage(k INTEGER); INSERT INTO empty_stage VALUES (7);")
        .unwrap();
    assert!(db
        .execute_batch("CREATE TABLE empty_stage (k INTEGER)")
        .is_err());
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM empty_stage", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn conditional_empty_create_fails_closed_without_existence_attestation() {
    let bundle = analyze("CREATE TABLE IF NOT EXISTS empty_stage (k INTEGER)");
    let plan = physical_ordered_write_state_plan(&bundle, &[]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}
