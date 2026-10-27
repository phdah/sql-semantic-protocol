//! TASK-145 source-free INSERT composition and transaction fail-closed boundaries.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ColumnValueKind, ColumnValueSource, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn state(rows: &[(i64, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new("target", rows.iter().map(|(id,score)| {
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".into(), LiteralValue::Number(id.to_string())),
            ("score".into(), LiteralValue::Number(score.to_string())),
        ])).unwrap()
    }).collect()).unwrap()
}

fn bundle(queries: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new(
        "target",
        ["id","score"].into_iter().map(|name| SchemaColumn::from_sql_type(name,"INTEGER","duckdb").unwrap()).collect(),
    ).unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = queries.iter().map(|sql| SqlInput::inline(*sql)).collect::<Vec<_>>();
    let names = (0..inputs.len()).map(|i| format!("task145-{i}")).collect::<Vec<_>>();
    let configured = inputs.iter().zip(&names).map(|(source, id)| {
        ConfiguredSqlInput::new(id, source, "duckdb", dialect.as_ref())
    }).collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured,&catalog).unwrap()
}

#[test]
fn sequential_values_defaults_and_delete_observe_prior_committed_target_state() {
    let statements = [
        "INSERT INTO target(id,score) VALUES (1,10),(2,20)",
        "INSERT INTO target DEFAULT VALUES",
        "DELETE FROM target WHERE id=1",
    ];
    let mut analysis = bundle(&statements);
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata,"task145-precedence").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    analysis.enrich_relation_constraints(&[
        RelationConstraintSet::new("target",vec![]).unwrap()
            .with_column_value_sources(vec![
                ColumnValueSource::new("id",ColumnValueKind::Default,Some(ConstraintValue::Integer(5)),evidence.clone()).unwrap(),
                ColumnValueSource::new("score",ColumnValueKind::Default,Some(ConstraintValue::Integer(7)),evidence).unwrap(),
            ]),
    ]);
    let plan = physical_ordered_write_state_plan(&analysis,&[state(&[(0,0)])]);
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } = plan else {
        panic!("exact sequential source-free write plan: {plan:?}");
    };
    assert_eq!(transitions.len(),3);
    assert_eq!(transitions[0].before().rows(),state(&[(0,0)]).rows());
    assert_eq!(transitions[0].after().rows(),state(&[(0,0),(1,10),(2,20)]).rows());
    assert_eq!(transitions[1].before(),transitions[0].after());
    assert_eq!(transitions[1].after().rows(),state(&[(0,0),(1,10),(2,20),(5,7)]).rows());
    assert_eq!(transitions[2].before(),transitions[1].after());
    assert_eq!(transitions[2].after().rows(),state(&[(0,0),(2,20),(5,7)]).rows());
    assert_eq!(transitions[0].affected().inserted(),2);
    assert_eq!(transitions[1].affected().inserted(),1);
    assert_eq!(transitions[2].affected().deleted(),1);
    assert_eq!(final_states[0].rows(),state(&[(0,0),(2,20),(5,7)]).rows());

    let db=Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE target(id INTEGER DEFAULT 5,score INTEGER DEFAULT 7); INSERT INTO target VALUES (0,0);").unwrap();
    for sql in statements { db.execute_batch(sql).unwrap(); }
    let mut stmt=db.prepare("SELECT id,score FROM target ORDER BY id").unwrap();
    let rows=stmt.query_map([],|r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?)))
        .unwrap().map(|r|r.unwrap()).collect::<Vec<_>>();
    assert_eq!(rows,vec![(0,0),(2,20),(5,7)]);
}

#[test]
fn unavailable_prestate_and_unproved_sql_transactions_cannot_be_exact() {
    let first = bundle(&["INSERT INTO target(id,score) VALUES (1,10)"]);
    assert!(matches!(
        physical_ordered_write_state_plan(&first,&[]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
    let transactional = bundle(&[
        "BEGIN TRANSACTION",
        "INSERT INTO target(id,score) VALUES (1,10)",
        "COMMIT",
    ]);
    assert!(!matches!(
        physical_ordered_write_state_plan(&transactional,&[state(&[])]),
        PhysicalWriteStatePlan::Feasible { .. }
    ));
}
