//! TASK-161: versioned physical source provenance starts from the caller's
//! actual complete bag, not a manufactured first write.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_sequential_state_graph_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalScriptProducer, PhysicalScriptSourceRole, PhysicalSequentialStateGraphProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn row(a: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([(
        "a".to_string(),
        a.map_or(LiteralValue::Null, |value| LiteralValue::Number(value.to_string())),
    )]))
    .unwrap()
}

#[test]
fn initial_attested_bag_is_read_from_initial_then_prior_write_without_inventing_rows() {
    let sql = [
        "SELECT a FROM t WHERE a IS NOT NULL",
        "UPDATE t SET a = 9 WHERE a = 1",
        "SELECT a FROM t",
    ];
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = sql.iter().map(|sql| SqlInput::inline(*sql)).collect::<Vec<_>>();
    let names = (0..input.len()).map(|n| format!("step-{n}")).collect::<Vec<_>>();
    let configured = input.iter().zip(&names)
        .map(|(sql, name)| ConfiguredSqlInput::new(name, sql, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "t",
            vec![SchemaColumn::from_sql_type("a", "INTEGER", "duckdb").unwrap()],
        ).unwrap(),
    ]).unwrap();
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let initial = PhysicalWriteState::new("t", vec![row(Some(1)), row(None), row(Some(1))]).unwrap();
    let proof = physical_sequential_state_graph_plan(&bundle, std::slice::from_ref(&initial));
    let PhysicalSequentialStateGraphProof::Exact(exact) = proof else {
        panic!("expected exact attested initial rows and versioned edges: {proof:?}");
    };
    assert_eq!(exact.nodes.len(), 3);
    assert_eq!(exact.nodes[0].before.states, vec![initial.clone()]);
    assert_eq!(exact.nodes[0].after.states, vec![initial.clone()]);
    assert_eq!(exact.nodes[0].read.as_ref().unwrap().surviving_source_ordinals, vec![0, 2]);
    assert_eq!(exact.nodes[0].read.as_ref().unwrap().rejected_source_ordinals, vec![1]);
    assert_eq!(exact.nodes[1].write.as_ref().unwrap().affected_target_ordinals(), &[0, 2]);
    assert_eq!(exact.nodes[2].after.states[0].rows(), &[row(Some(9)), row(None), row(Some(9))]);
    assert!(exact.edges.iter().any(|edge| {
        edge.relation == "t"
            && edge.consumer_layer_id == exact.nodes[0].layer_id
            && edge.producer == PhysicalScriptProducer::Initial
            && edge.role == PhysicalScriptSourceRole::ReadSource
    }));
    assert!(exact.edges.iter().any(|edge| {
        edge.relation == "t"
            && edge.consumer_layer_id == exact.nodes[1].layer_id
            && edge.producer == PhysicalScriptProducer::Initial
            && edge.role == PhysicalScriptSourceRole::TargetPrestate
    }));
    assert!(exact.edges.iter().any(|edge| {
        edge.relation == "t"
            && edge.consumer_layer_id == exact.nodes[2].layer_id
            && edge.producer == PhysicalScriptProducer::Statement(exact.nodes[1].layer_id.clone())
            && edge.role == PhysicalScriptSourceRole::ReadSource
    }));

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE t(a INTEGER); INSERT INTO t VALUES (1), (NULL), (1)")
        .unwrap();
    let native_before = db.prepare("SELECT a FROM t ORDER BY rowid").unwrap()
        .query_map([], |r| r.get::<_, Option<i64>>(0)).unwrap()
        .collect::<duckdb::Result<Vec<_>>>().unwrap();
    assert_eq!(native_before, vec![Some(1), None, Some(1)]);
    db.execute_batch(sql[1]).unwrap();
    let native_after = db.prepare("SELECT a FROM t ORDER BY rowid").unwrap()
        .query_map([], |r| r.get::<_, Option<i64>>(0)).unwrap()
        .collect::<duckdb::Result<Vec<_>>>().unwrap();
    assert_eq!(native_after, vec![Some(9), None, Some(9)]);
    assert_eq!(exact.final_states[0].rows(), &[row(Some(9)), row(None), row(Some(9))]);
}
