//! Nested Arrow-like complete populations share the scalar relation constraint verifier.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_typed_closed_world_schema_plan, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind, DataType, DataTypeField,
    LiteralValue, PhysicalTypedPopulationPlan, PhysicalTypedValue, PhysicalTypedWriteRow,
    PhysicalTypedWriteState, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput,
};

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "warehouse").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn value(n: i64) -> PhysicalTypedValue {
    PhysicalTypedValue::Scalar(LiteralValue::Number(n.to_string()))
}

fn typed_row(id: i64, payload: PhysicalTypedValue) -> PhysicalTypedWriteRow {
    PhysicalTypedWriteRow::new(BTreeMap::from([
        ("id".into(), value(id)),
        ("payload".into(), payload),
    ]))
    .unwrap()
}

fn typed_state(relation: &str, rows: Vec<PhysicalTypedWriteRow>) -> PhysicalTypedWriteState {
    PhysicalTypedWriteState::new(relation, rows).unwrap()
}

fn bundle() -> sql_semantic_protocol::AnalysisBundle {
    let payload = DataType::Struct {
        fields: vec![
            DataTypeField::new(
                Some("scores".into()),
                DataType::Array {
                    element: Some(Box::new(DataType::SignedInteger { bits: Some(32) })),
                    length: Some(2),
                },
            ),
            DataTypeField::new(
                Some("label".into()),
                DataType::String {
                    length: Some(8),
                    fixed: false,
                },
            ),
        ],
    };
    let schemas = ["target", "source"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::new("id", DataType::SignedInteger { bits: Some(32) }).unwrap(),
                    SchemaColumn::new("payload", payload.clone()).unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let sql = SqlInput::inline("SELECT id FROM target");
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = ConfiguredSqlInput::new("typed", &sql, "duckdb", dialect.as_ref());
    let mut result = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
    result.enrich_relation_constraints(&[
        RelationConstraintSet::new(
            "target",
            vec![
                RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap(),
                RelationConstraint::foreign_key(
                    vec!["id".into()], "source", vec!["id".into()], evidence(),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        RelationConstraintSet::new(
            "source",
            vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
        )
        .unwrap(),
    ]);
    result
}

fn payload(left: i64, right: i64) -> PhysicalTypedValue {
    PhysicalTypedValue::Struct(vec![
        PhysicalTypedValue::Array(vec![value(left), value(right)]),
        PhysicalTypedValue::Scalar(LiteralValue::Text("ok".into())),
    ])
}

#[test]
fn typed_nesting_preserves_rows_and_checks_cross_source_fk() {
    let bundle = bundle();
    let input = [
        typed_state("target", vec![typed_row(1, payload(2, 3))]),
        typed_state("source", vec![typed_row(1, payload(5, 6))]),
    ];
    let result = physical_typed_closed_world_schema_plan(&bundle, &input);
    let PhysicalTypedPopulationPlan::Feasible { final_states } = result else {
        panic!("expected jointly feasible physical populations: {result:?}");
    };
    assert_eq!(final_states.len(), 2);
    assert_eq!(final_states[1].rows()[0].values()["payload"], payload(2, 3));

    let invalid = [
        typed_state("target", vec![typed_row(1, payload(2, 3)), typed_row(1, payload(4, 5))]),
        typed_state("source", vec![typed_row(1, payload(5, 6))]),
    ];
    assert!(matches!(
        physical_typed_closed_world_schema_plan(&bundle, &invalid),
        PhysicalTypedPopulationPlan::Impossible { .. }
    ));
}

#[test]
fn typed_population_nested_shape_out_of_range_and_unresolved_fk_fail_closed() {
    let bundle = bundle();
    let malformed = [
        typed_state("target", vec![typed_row(1, PhysicalTypedValue::Struct(vec![value(1)]))]),
        typed_state("source", vec![typed_row(1, payload(1, 2))]),
    ];
    assert!(matches!(
        physical_typed_closed_world_schema_plan(&bundle, &malformed),
        PhysicalTypedPopulationPlan::Impossible { .. }
    ));

    let overflow = [
        typed_state("target", vec![typed_row(1, payload(i64::MAX, 2))]),
        typed_state("source", vec![typed_row(1, payload(1, 2))]),
    ];
    assert!(matches!(
        physical_typed_closed_world_schema_plan(&bundle, &overflow),
        PhysicalTypedPopulationPlan::Impossible { .. }
    ));

    let missing_fk = [typed_state("target", vec![typed_row(1, payload(1, 2))])];
    assert!(matches!(
        physical_typed_closed_world_schema_plan(&bundle, &missing_fk),
        PhysicalTypedPopulationPlan::Residual { .. }
    ));
}

#[test]
fn physical_nested_population_duckdb_array_struct_differential() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE events (id INTEGER PRIMARY KEY, payload STRUCT(scores INTEGER[2], label VARCHAR));\n\
         INSERT INTO events VALUES (1, {'scores': [2,3], 'label': 'ok'});",
    )
    .unwrap();
    let first: i32 = db
        .query_row("SELECT payload.scores[1] FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(first, 2);
    assert!(db
        .execute_batch(
            "INSERT INTO events VALUES (1, {'scores': [4,5], 'label': 'duplicate'});",
        )
        .is_err());
}
