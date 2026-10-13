//! Nested Arrow-like complete populations share the scalar relation constraint verifier.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_typed_append_schema_plan,
    physical_typed_closed_world_schema_plan, physical_typed_ordered_insert_plan,
    physical_typed_ordered_write_plan,
    CheckCompareOperator, ConfiguredSqlInput,
    ComputedBooleanCheckConstraint, ComputedBooleanCheckPredicate,
    ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
    DataType, DataTypeField, GeneratedValueExpression, LiteralValue,
    PhysicalTypedPopulationPlan, PhysicalTypedValue,
    PhysicalTypedWriteRow, PhysicalTypedWriteState, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
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
                    vec!["id".into()],
                    "source",
                    vec!["id".into()],
                    evidence(),
                )
                .unwrap(),
                RelationConstraint::not_null("payload", evidence()).unwrap(),
            ],
        )
        .unwrap(),
        RelationConstraintSet::new(
            "source",
            vec![
                RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap(),
                RelationConstraint::not_null("payload", evidence()).unwrap(),
            ],
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
        typed_state(
            "target",
            vec![typed_row(1, payload(2, 3)), typed_row(1, payload(4, 5))],
        ),
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
        typed_state(
            "target",
            vec![typed_row(1, PhysicalTypedValue::Struct(vec![value(1)]))],
        ),
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

    let null_nested = [
        typed_state(
            "target",
            vec![typed_row(1, PhysicalTypedValue::Scalar(LiteralValue::Null))],
        ),
        typed_state("source", vec![typed_row(1, payload(1, 2))]),
    ];
    assert!(matches!(
        physical_typed_closed_world_schema_plan(&bundle, &null_nested),
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
        .execute_batch("INSERT INTO events VALUES (1, {'scores': [4,5], 'label': 'duplicate'});",)
        .is_err());
    // The same structured value is returned as an actual Arrow StructArray,
    // not a serialized SQL text representation.
    let mut arrow_statement = db.prepare("SELECT payload FROM events").unwrap();
    let mut batches = arrow_statement.query_arrow([]).unwrap();
    let arrow_batch = batches.next().unwrap();
    assert_eq!(arrow_batch.num_rows(), 1);
    assert_eq!(arrow_batch.num_columns(), 1);
    assert!(format!("{:?}", arrow_batch.schema().field(0).data_type()).contains("Struct"));
}

#[test]
fn reversed_scalar_check_comparison_normalizes_across_parser_dialects() {
    let schema = RelationSchema::new(
        "target",
        vec![SchemaColumn::new("id", DataType::SignedInteger { bits: Some(32) }).unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    for name in [
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
    ] {
        let dialect = dialect_from_name(name).unwrap();
        let sql = SqlInput::inline("CREATE TABLE target(id INTEGER, CHECK (3 < id))");
        let input = ConfiguredSqlInput::new("check", &sql, name, dialect.as_ref());
        let Ok(bundle) = analyze_configured_inputs_with_catalog(&[input], &catalog) else {
            // Explicit parser boundary for dialects rejecting the shared DDL.
            continue;
        };
        let set = bundle
            .relation_constraints()
            .iter()
            .find(|set| set.relation() == "target")
            .expect("DDL CHECK relation constraints");
        assert!(
            set.constraints().iter().any(|rule| matches!(rule,
                RelationConstraint::CheckComparison(c)
                    if c.column() == "id"
                       && c.operator() == CheckCompareOperator::Gt
            )),
            "{name}: {set:?}"
        );
    }
}

#[test]
fn typed_json_populations_preserve_document_null_under_cross_source_constraints() {
    let schemas = ["target", "source"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                vec![
                    SchemaColumn::new("id", DataType::SignedInteger { bits: Some(32) }).unwrap(),
                    SchemaColumn::new("payload", DataType::Json).unwrap(),
                ],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let sql = SqlInput::inline("SELECT id FROM target");
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = ConfiguredSqlInput::new("typed_json", &sql, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
    bundle.enrich_relation_constraints(&[
        RelationConstraintSet::new(
            "target",
            vec![
                RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap(),
                RelationConstraint::foreign_key(
                    vec!["id".into()],
                    "source",
                    vec!["id".into()],
                    evidence(),
                )
                .unwrap(),
                RelationConstraint::not_null("payload", evidence()).unwrap(),
            ],
        )
        .unwrap(),
        RelationConstraintSet::new(
            "source",
            vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
        )
        .unwrap(),
    ]);
    let target = typed_state(
        "target",
        vec![typed_row(
            1,
            PhysicalTypedValue::Json(serde_json::json!({"status": "ok", "items": [null, 3]})),
        )],
    );
    let source = typed_state(
        "source",
        vec![typed_row(
            1,
            PhysicalTypedValue::Json(serde_json::Value::Null),
        )],
    );
    let proof = physical_typed_closed_world_schema_plan(&bundle, &[target.clone(), source.clone()]);
    assert!(
        matches!(proof, PhysicalTypedPopulationPlan::Feasible { .. }),
        "{proof:?}"
    );

    let invalid = typed_state(
        "target",
        vec![typed_row(1, PhysicalTypedValue::Scalar(LiteralValue::Null))],
    );
    assert!(matches!(
        physical_typed_closed_world_schema_plan(&bundle, &[invalid, source]),
        PhysicalTypedPopulationPlan::Impossible { .. }
    ));
    let fractional = typed_state(
        "target",
        vec![typed_row(
            1,
            PhysicalTypedValue::Json(serde_json::json!({"amount": 0.1})),
        )],
    );
    assert!(matches!(
        physical_typed_closed_world_schema_plan(
            &bundle,
            &[
                fractional,
                typed_state(
                    "source",
                    vec![typed_row(
                        1,
                        PhysicalTypedValue::Json(serde_json::Value::Null)
                    )]
                )
            ]
        ),
        PhysicalTypedPopulationPlan::Residual { .. }
    ));
}

#[test]
fn typed_target_append_preserves_nested_payload_and_proves_existing_key_and_fk_laws() {
    let bundle = bundle();
    let original = [
        typed_state("target", vec![typed_row(1, payload(2, 3))]),
        typed_state(
            "source",
            vec![typed_row(1, payload(5, 6)), typed_row(2, payload(7, 8))],
        ),
    ];
    let inserted = typed_row(2, payload(3, 4));
    let result = physical_typed_append_schema_plan(
        &bundle,
        &original,
        "target",
        std::slice::from_ref(&inserted),
    );
    let PhysicalTypedPopulationPlan::Feasible { final_states } = result else {
        panic!("expected feasible typed append: {result:?}");
    };
    let target = final_states
        .iter()
        .find(|state| state.relation() == "target")
        .unwrap();
    assert_eq!(target.rows().len(), 2);
    assert_eq!(target.rows()[0].values()["payload"], payload(2, 3));
    assert_eq!(target.rows()[1], inserted);

    assert!(matches!(
        physical_typed_append_schema_plan(
            &bundle,
            &original,
            "target",
            &[typed_row(1, payload(9, 10))],
        ),
        PhysicalTypedPopulationPlan::Impossible { .. }
    ));
    assert!(matches!(
        physical_typed_append_schema_plan(
            &bundle,
            &original,
            "target",
            &[typed_row(3, payload(9, 10))],
        ),
        PhysicalTypedPopulationPlan::Residual { .. }
    ));
    assert!(matches!(
        physical_typed_append_schema_plan(
            &bundle,
            &original,
            "target",
            &[typed_row(2, PhysicalTypedValue::Scalar(LiteralValue::Null))],
        ),
        PhysicalTypedPopulationPlan::Impossible { .. }
    ));
}

#[test]
fn typed_target_append_requires_a_complete_initial_target_and_reference_population() {
    let bundle = bundle();
    let missing_target = [typed_state("source", vec![typed_row(2, payload(3, 4))])];
    assert!(matches!(
        physical_typed_append_schema_plan(
            &bundle,
            &missing_target,
            "target",
            &[typed_row(2, payload(3, 4))],
        ),
        PhysicalTypedPopulationPlan::Residual { .. }
    ));
    let missing_parent = [typed_state("target", vec![typed_row(1, payload(2, 3))])];
    assert!(matches!(
        physical_typed_append_schema_plan(
            &bundle,
            &missing_parent,
            "target",
            &[typed_row(2, payload(3, 4))],
        ),
        PhysicalTypedPopulationPlan::Residual { .. }
    ));
}

#[test]
fn ddl_disjunction_normalizes_only_full_portable_same_column_equality() {
    let names = [
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
    let schema = RelationSchema::new(
        "target",
        vec![
            SchemaColumn::new("id", DataType::SignedInteger { bits: Some(32) }).unwrap(),
            SchemaColumn::new("score", DataType::SignedInteger { bits: Some(32) }).unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let mut recognized = 0_usize;
    for name in names {
        let dialect = dialect_from_name(name).unwrap();
        let sql = SqlInput::inline(
            "CREATE TABLE target (id INTEGER, score INTEGER, CHECK (id = 1 OR id = 2))",
        );
        let input = ConfiguredSqlInput::new("or_check", &sql, name, dialect.as_ref());
        let Ok(bundle) = analyze_configured_inputs_with_catalog(&[input], &catalog) else {
            // Parser-boundary rejection is not a successful semantic proof.
            continue;
        };
        recognized += 1;
        let set = bundle
            .relation_constraints()
            .iter()
            .find(|set| set.relation() == "target")
            .expect("analyzed DDL must carry constraint facts");
        assert!(
            set.constraints().iter().any(|rule| matches!(
                rule,
                RelationConstraint::AcceptedValues(values)
                    if values.column() == "id"
                        && values.values().len() == 2
            )),
            "{name}: {set:?}"
        );
        assert!(!set
            .diagnostics()
            .iter()
            .any(|d| d.code() == "unsupported_check_constraint"));
    }
    assert!(
        recognized > 0,
        "no dialect recognized the positive SQL DDL fixture"
    );

    let dialect = dialect_from_name("duckdb").unwrap();
    for unsupported in [
        "CHECK (id = 1 OR score = 2)",
        "CHECK (id = 1 OR id = NULL)",
        "CHECK (id = 1 OR lower(score) = 2)",
    ] {
        let sql = SqlInput::inline(format!(
            "CREATE TABLE target (id INTEGER, score INTEGER, {unsupported})"
        ));
        let input = ConfiguredSqlInput::new("unsupported_or", &sql, "duckdb", dialect.as_ref());
        let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
        let set = bundle
            .relation_constraints()
            .iter()
            .find(|set| set.relation() == "target")
            .unwrap();
        assert!(
            set.diagnostics()
                .iter()
                .any(|d| d.code() == "unsupported_check_constraint"),
            "{unsupported}: {set:?}"
        );
        assert!(
            !set.constraints()
                .iter()
                .any(|rule| matches!(rule, RelationConstraint::AcceptedValues(_))),
            "{unsupported} silently dropped an unproved OR branch"
        );
    }
}

fn typed_insert_bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let prior = bundle();
    let catalog = RelationCatalog::from_schemas(prior.source_schemas()).unwrap();
    let source = SqlInput::inline(sql);
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = ConfiguredSqlInput::new("typed-insert", &source, "duckdb", dialect.as_ref());
    let mut result = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
    result.enrich_relation_constraints(prior.relation_constraints());
    result
}

#[test]
fn ordered_typed_insert_select_preserves_exact_nested_values() {
    let sql = "INSERT INTO target (id,payload) SELECT id,payload FROM source";
    let bundle = typed_insert_bundle(sql);
    let snapshots = [
        typed_state("target", vec![]),
        typed_state("source", vec![
            typed_row(1, payload(2,3)), typed_row(2,payload(4,5))
        ]),
    ];
    let plan = physical_typed_ordered_insert_plan(&bundle, &snapshots);
    let PhysicalTypedPopulationPlan::Feasible { final_states } = plan else {
        panic!("expected typed INSERT proof: {plan:?}");
    };
    let target = final_states.iter().find(|s| s.relation()=="target").unwrap();
    assert_eq!(target.rows(), snapshots[1].rows());

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE source(
         id INTEGER PRIMARY KEY,
         payload STRUCT(scores INTEGER[2], label VARCHAR));
         CREATE TABLE target(
         id INTEGER PRIMARY KEY,
         payload STRUCT(scores INTEGER[2], label VARCHAR));
         INSERT INTO source VALUES
         (1, {'scores':[2,3], 'label':'ok'}),
         (2, {'scores':[4,5], 'label':'ok'});
         INSERT INTO target(id,payload) SELECT id,payload FROM source;").unwrap();
    let count: i64 = db.query_row("SELECT COUNT(*) FROM target",[],|row| row.get(0)).unwrap();
    assert_eq!(count,2);
    let mut statement = db.prepare("SELECT payload FROM target ORDER BY id").unwrap();
    let mut batches = statement.query_arrow([]).unwrap();
    let batch = batches.next().unwrap();
    assert!(format!("{:?}",batch.schema().field(0).data_type()).contains("Struct"));
}

#[test]
fn ordered_typed_insert_rejects_existing_key_collisions_and_unknown_sql() {
    let complete = typed_insert_bundle(
        "INSERT INTO target (id,payload) SELECT id,payload FROM source",
    );
    let collisions = [
        typed_state("target",vec![typed_row(1,payload(8,9))]),
        typed_state("source",vec![typed_row(1,payload(2,3))]),
    ];
    assert!(matches!(
        physical_typed_ordered_insert_plan(&complete,&collisions),
        PhysicalTypedPopulationPlan::Impossible { .. }
    ));
    for sql in [
        "INSERT INTO target(id,payload) SELECT id+1,payload FROM source",
        "INSERT INTO target(id,payload) SELECT id,payload FROM source WHERE payload IS NULL",
        "INSERT INTO target(id,payload) SELECT id,payload FROM source WHERE payload IS NOT NULL",
    ] {
        let input = [
            typed_state("target",vec![]),
            typed_state("source",vec![typed_row(1,payload(2,3))]),
        ];
        assert!(
            matches!(
                physical_typed_ordered_insert_plan(&typed_insert_bundle(sql),&input),
                PhysicalTypedPopulationPlan::Residual { .. }
            ),
            "{sql}"
        );
    }
}

#[test]
fn filtered_typed_insert_proves_scalar_truth_and_preserves_nested_arrow_values() {
    let statement = "INSERT INTO target (id,payload) SELECT id,payload FROM source WHERE id >= 2";
    let snapshots = [
        typed_state("target", vec![]),
        typed_state(
            "source",
            vec![
                typed_row(1, payload(2, 3)),
                typed_row(2, payload(4, 5)),
                typed_row(3, payload(6, 7)),
            ],
        ),
    ];
    let plan = physical_typed_ordered_insert_plan(&typed_insert_bundle(statement), &snapshots);
    let PhysicalTypedPopulationPlan::Feasible { final_states } = plan else {
        panic!("scalar WHERE must preserve exact nested rows: {plan:?}");
    };
    let target = final_states
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    assert_eq!(
        target.rows(),
        &[typed_row(2, payload(4, 5)), typed_row(3, payload(6, 7))]
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER PRIMARY KEY, payload STRUCT(scores INTEGER[2], label VARCHAR));
         CREATE TABLE target(id INTEGER PRIMARY KEY, payload STRUCT(scores INTEGER[2], label VARCHAR));
         INSERT INTO source VALUES
           (1, {'scores':[2,3], 'label':'ok'}),
           (2, {'scores':[4,5], 'label':'ok'}),
           (3, {'scores':[6,7], 'label':'ok'});
         INSERT INTO target (id,payload) SELECT id,payload FROM source WHERE id >= 2;",
    )
    .unwrap();
    let mut statement = db.prepare("SELECT id FROM target ORDER BY id").unwrap();
    let ids = statement
        .query_map([], |row| row.get::<_, i32>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(ids, vec![2, 3]);

    let empty = physical_typed_ordered_insert_plan(
        &typed_insert_bundle(
            "INSERT INTO target (id,payload) SELECT id,payload FROM source WHERE id IS NULL",
        ),
        &snapshots,
    );
    let PhysicalTypedPopulationPlan::Feasible { final_states } = empty else {
        panic!("SQL UNKNOWN/FALSE selection must produce an empty bag: {empty:?}");
    };
    assert!(final_states
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap()
        .rows()
        .is_empty());
}

#[test]
fn ordered_typed_scalar_update_and_delete_preserve_nested_fields_and_foreign_keys() {
    let snapshots = [
        typed_state("target", vec![typed_row(1, payload(2, 3))]),
        typed_state(
            "source",
            vec![typed_row(1, payload(4, 5)), typed_row(11, payload(6, 7))],
        ),
    ];
    let update = typed_insert_bundle("UPDATE target SET id=id+10 WHERE id=1");
    let first = physical_typed_ordered_write_plan(&update, &snapshots);
    let PhysicalTypedPopulationPlan::Feasible { final_states } = first else {
        panic!("scalar UPDATE should preserve untouched nested value: {first:?}");
    };
    let target = final_states
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    assert_eq!(target.rows(), &[typed_row(11, payload(2, 3))]);

    let combined =
        typed_insert_bundle("UPDATE target SET id=id+10 WHERE id=1; DELETE FROM target WHERE id=11");
    let plan = physical_typed_ordered_write_plan(&combined, &snapshots);
    let PhysicalTypedPopulationPlan::Feasible { final_states } = plan else {
        panic!("ordered typed mutation proof failed: {plan:?}");
    };
    assert!(final_states.iter().find(|s| s.relation() == "target").unwrap().rows().is_empty());

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER PRIMARY KEY, payload STRUCT(scores INTEGER[2], label VARCHAR));
         CREATE TABLE target(id INTEGER PRIMARY KEY, payload STRUCT(scores INTEGER[2], label VARCHAR));
         INSERT INTO source VALUES
            (1, {'scores':[4,5], 'label':'ok'}),
            (11, {'scores':[6,7], 'label':'ok'});
         INSERT INTO target VALUES (1, {'scores':[2,3], 'label':'ok'});
         UPDATE target SET id=id+10 WHERE id=1;
         DELETE FROM target WHERE id=11;",
    )
    .unwrap();
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM target", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);

    assert!(matches!(
        physical_typed_ordered_insert_plan(&combined, &snapshots),
        PhysicalTypedPopulationPlan::Residual { .. },
    ));
}

#[test]
fn typed_mutations_reject_nested_sql_evaluation_and_final_fk_contradictions() {
    let snapshots = [
        typed_state("target", vec![typed_row(1, payload(2, 3))]),
        typed_state("source", vec![typed_row(1, payload(4, 5))]),
    ];
    for statement in [
        "UPDATE target SET payload=NULL WHERE id=1",
        "DELETE FROM target WHERE payload IS NOT NULL",
        "UPDATE target SET id=2 WHERE payload IS NULL",
    ] {
        let plan = physical_typed_ordered_write_plan(&typed_insert_bundle(statement), &snapshots);
        assert!(
            matches!(plan, PhysicalTypedPopulationPlan::Residual { .. }),
            "{statement} must not reinterpret an Arrow struct as scalar NULL: {plan:?}",
        );
    }
    let plan = physical_typed_ordered_write_plan(
        &typed_insert_bundle("UPDATE target SET id=100 WHERE id=1"),
        &snapshots,
    );
    assert!(
        matches!(plan, PhysicalTypedPopulationPlan::Impossible { .. }),
        "a new scalar FK violation must be checked against the complete source: {plan:?}",
    );
}

#[test]
fn nested_boolean_check_must_not_evaluate_a_private_null_placeholder() {
    let mut bundle = bundle();
    bundle.enrich_relation_constraints(&[
        RelationConstraintSet::new("target", vec![]).unwrap()
            .with_computed_boolean_checks(vec![
                ComputedBooleanCheckConstraint::new(
                    ComputedBooleanCheckPredicate::IsNull {
                        expression: GeneratedValueExpression::Column("payload".into()),
                        negated:false,
                    },
                    evidence(),
                ).unwrap(),
            ]),
    ]);
    let snapshots = [
        typed_state("target", vec![typed_row(1,payload(2,3))]),
        typed_state("source", vec![typed_row(1,payload(4,5))]),
    ];
    assert!(matches!(
        physical_typed_closed_world_schema_plan(&bundle,&snapshots),
        PhysicalTypedPopulationPlan::Residual { .. }
    ));
}
