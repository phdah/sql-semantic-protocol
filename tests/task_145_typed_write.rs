//! Source-free INSERT literal/default proof over the Arrow-like typed physical API.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_typed_ordered_write_plan,
    ColumnValueKind, ColumnValueSource, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind, ConstraintValue,
    GeneratedValueExpression, LiteralValue, PhysicalTypedPopulationPlan, PhysicalTypedValue,
    PhysicalTypedWriteRow, PhysicalTypedWriteState, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task145-typed").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn typed_row(id: Option<i64>, score: Option<i64>) -> PhysicalTypedWriteRow {
    let scalar = |value: Option<i64>| {
        PhysicalTypedValue::Scalar(
            value.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
        )
    };
    PhysicalTypedWriteRow::new(BTreeMap::from([
        ("id".to_string(), scalar(id)),
        ("score".to_string(), scalar(score)),
    ]))
    .unwrap()
}

fn bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new(
        "target",
        ["id", "score"]
            .into_iter()
            .map(|c| SchemaColumn::from_sql_type(c, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline(sql);
    let configured = ConfiguredSqlInput::new("task145-typed", &sql, "duckdb", dialect.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap()
}

fn proof(
    bundle: &sql_semantic_protocol::AnalysisBundle,
    initial: Vec<PhysicalTypedWriteRow>,
) -> PhysicalTypedPopulationPlan {
    physical_typed_ordered_write_plan(
        bundle,
        &[PhysicalTypedWriteState::new("target", initial).unwrap()],
    )
}

fn target(plan: PhysicalTypedPopulationPlan) -> Vec<PhysicalTypedWriteRow> {
    let PhysicalTypedPopulationPlan::Feasible { final_states } = plan else {
        panic!("expected exact typed INSERT state, got {plan:?}");
    };
    final_states
        .into_iter()
        .find(|s| s.relation() == "target")
        .unwrap()
        .rows()
        .to_vec()
}

#[test]
fn typed_values_keep_duplicate_and_null_rows_without_inferred_casts() {
    let query = bundle("INSERT INTO target(id,score) VALUES (1,10),(1,10),(2,NULL)");
    assert_eq!(
        target(proof(&query, vec![typed_row(Some(0), Some(0))])),
        vec![
            typed_row(Some(0), Some(0)),
            typed_row(Some(1), Some(10)),
            typed_row(Some(1), Some(10)),
            typed_row(Some(2), None),
        ],
    );
}

#[test]
fn typed_default_values_and_omitted_fields_require_attested_values() {
    let mut omitted = bundle("INSERT INTO target(id) VALUES (1),(2)");
    let score = ColumnValueSource::new(
        "score",
        ColumnValueKind::Default,
        Some(ConstraintValue::Integer(7)),
        evidence(),
    )
    .unwrap();
    omitted.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
        .unwrap()
        .with_column_value_sources(vec![score.clone()])]);
    assert_eq!(
        target(proof(&omitted, vec![])),
        vec![typed_row(Some(1), Some(7)), typed_row(Some(2), Some(7))]
    );
    let mut defaults = bundle("INSERT INTO target DEFAULT VALUES");
    let id = ColumnValueSource::new(
        "id",
        ColumnValueKind::Default,
        Some(ConstraintValue::Integer(5)),
        evidence(),
    )
    .unwrap();
    defaults.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
        .unwrap()
        .with_column_value_sources(vec![id, score])]);
    assert_eq!(
        target(proof(&defaults, vec![])),
        vec![typed_row(Some(5), Some(7))]
    );
    assert!(matches!(
        proof(&bundle("INSERT INTO target DEFAULT VALUES"), vec![]),
        PhysicalTypedPopulationPlan::Residual { .. }
    ));
}

#[test]
fn typed_insert_rejects_pk_collision_without_committing_any_partial_rows() {
    let mut query = bundle("INSERT INTO target(id,score) VALUES (1,10),(1,20)");
    query.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
    )
    .unwrap()]);
    assert!(matches!(
        proof(&query, vec![]),
        PhysicalTypedPopulationPlan::Impossible { .. }
    ));
}

#[test]
fn typed_insert_constructs_attested_generated_columns_from_scalar_inputs() {
    let mut query = bundle("INSERT INTO target(id) VALUES (2),(3)");
    let expression = GeneratedValueExpression::Add(
        Box::new(GeneratedValueExpression::Column("id".to_string())),
        Box::new(GeneratedValueExpression::Literal(ConstraintValue::Integer(
            1,
        ))),
    );
    query.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
        .unwrap()
        .with_column_value_sources(vec![ColumnValueSource::new(
            "score",
            ColumnValueKind::Generated,
            None,
            evidence(),
        )
        .unwrap()
        .with_generated_expression(expression)
        .unwrap()])]);
    assert_eq!(
        target(proof(&query, vec![typed_row(Some(0), Some(0))])),
        vec![
            typed_row(Some(0), Some(0)),
            typed_row(Some(2), Some(3)),
            typed_row(Some(3), Some(4)),
        ]
    );
}

#[test]
fn typed_insert_does_not_invent_generated_values_from_missing_dependencies() {
    let mut query = bundle("INSERT INTO target(id) VALUES (2)");
    query.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
        .unwrap()
        .with_column_value_sources(vec![ColumnValueSource::new(
            "score",
            ColumnValueKind::Generated,
            None,
            evidence(),
        )
        .unwrap()
        .with_generated_expression(GeneratedValueExpression::Column(
            "unattested_column".to_string(),
        ))
        .unwrap()])]);
    assert!(matches!(
        proof(&query, vec![]),
        PhysicalTypedPopulationPlan::Residual { .. }
    ));
}
