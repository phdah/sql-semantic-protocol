//! Closed-world ordered write construction and DuckDB statement oracles.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_closed_world_schema_plan,
    physical_ordered_write_state_plan, physical_typed_value_plan, AnalysisBundle,
    CheckCompareOperator, ColumnValueKind, ColumnValueSource, ConfiguredSqlInput,
    ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
    ConstraintValue, DataType, DataTypeField, LiteralValue, PhysicalTypeProof, PhysicalTypedValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};

fn bundle(sql: &[&str], dialect: &str) -> AnalysisBundle {
    let schemas = ["target", "source"]
        .into_iter()
        .map(|relation| {
            RelationSchema::new(
                relation,
                ["id", "score"]
                    .into_iter()
                    .map(|column| {
                        SchemaColumn::from_sql_type(column, "INTEGER", "postgresql").unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let parsed = sql
        .iter()
        .map(|text| SqlInput::inline(*text))
        .collect::<Vec<_>>();
    let names = (0..sql.len()).map(|n| format!("q-{n}")).collect::<Vec<_>>();
    let dialect_impl = dialect_from_name(dialect).unwrap();
    let inputs = parsed
        .iter()
        .zip(&names)
        .map(|(source, name)| ConfiguredSqlInput::new(name, source, dialect, dialect_impl.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&inputs, &catalog).unwrap()
}

fn row(id: Option<i64>, score: Option<i64>) -> PhysicalWriteRow {
    let values = [
        (
            "id".to_string(),
            id.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
        (
            "score".to_string(),
            score.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    PhysicalWriteRow::new(values).unwrap()
}

fn state(relation: &str, rows: &[(Option<i64>, Option<i64>)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter().map(|&(id, score)| row(id, score)).collect(),
    )
    .unwrap()
}

fn outcome(
    plan: &PhysicalWriteStatePlan,
) -> (
    &[sql_semantic_protocol::PhysicalWriteTransition],
    &[PhysicalWriteState],
) {
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("expected a complete construction, got {plan:?}");
    };
    (transitions, final_states)
}

#[test]
fn sequential_delete_update_delete_reuses_actual_prior_target_state() {
    let queries = [
        "DELETE FROM target WHERE score IS NULL",
        "UPDATE target SET score = score + 2 WHERE id = 1",
        "DELETE FROM target WHERE id > 2",
    ];
    for dialect in ["generic", "postgresql", "duckdb", "snowflake"] {
        let b = bundle(&queries, dialect);
        let plan = physical_ordered_write_state_plan(
            &b,
            &[state(
                "target",
                &[(Some(1), Some(4)), (Some(2), None), (Some(3), Some(8))],
            )],
        );
        let (steps, final_states) = outcome(&plan);
        assert_eq!(steps.len(), 3, "{dialect}");
        assert_eq!(steps[0].affected().deleted(), 1);
        assert_eq!(steps[1].affected().updated(), 1);
        assert_eq!(steps[2].affected().deleted(), 1);
        assert_eq!(steps[1].before().rows(), steps[0].after().rows());
        assert_eq!(steps[2].before().rows(), steps[1].after().rows());
        assert_eq!(final_states[0].rows(), &[row(Some(1), Some(6))]);
    }

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER, score INTEGER);
         INSERT INTO target VALUES (1,4), (2,NULL), (3,8);
         DELETE FROM target WHERE score IS NULL;
         UPDATE target SET score = score + 2 WHERE id = 1;
         DELETE FROM target WHERE id > 2;",
    )
    .unwrap();
    let observed: (i64, i64) = db
        .query_row("SELECT id,score FROM target", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(observed, (1, 6));
}

#[test]
fn insert_select_uses_complete_source_and_preserves_duplicate_rows() {
    let b = bundle(
        &["INSERT INTO target (id, score) SELECT id, score FROM source WHERE id > 1"],
        "duckdb",
    );
    let plan = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[(Some(1), Some(1))]),
            state(
                "source",
                &[(Some(1), Some(8)), (Some(2), Some(9)), (Some(2), Some(9))],
            ),
        ],
    );
    let (steps, states) = outcome(&plan);
    assert_eq!(steps[0].kind(), WriteKind::Append);
    assert_eq!(steps[0].affected().inserted(), 2);
    let result = states.iter().find(|s| s.relation() == "target").unwrap();
    assert_eq!(
        result.rows(),
        &[
            row(Some(1), Some(1)),
            row(Some(2), Some(9)),
            row(Some(2), Some(9))
        ]
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER, score INTEGER);
        CREATE TABLE source(id INTEGER, score INTEGER);
        INSERT INTO target VALUES (1,1);
        INSERT INTO source VALUES (1,8),(2,9),(2,9);
        INSERT INTO target (id,score) SELECT id,score FROM source WHERE id>1;",
    )
    .unwrap();
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM target WHERE id = 2", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);
}

#[test]
fn merge_first_applicable_branches_are_realized_once_per_row() {
    let sql = "MERGE INTO target USING source ON target.id = source.id
       WHEN MATCHED AND source.score < 0 THEN DELETE
       WHEN MATCHED THEN UPDATE SET score = source.score
       WHEN NOT MATCHED THEN INSERT (id,score) VALUES (source.id,source.score)";
    let b = bundle(&[sql], "duckdb");
    let plan = physical_ordered_write_state_plan(
        &b,
        &[
            state(
                "target",
                &[(Some(1), Some(2)), (Some(2), Some(3)), (Some(4), Some(4))],
            ),
            state(
                "source",
                &[
                    (Some(1), Some(-5)),
                    (Some(2), Some(20)),
                    (Some(3), Some(30)),
                ],
            ),
        ],
    );
    let (steps, states) = outcome(&plan);
    assert_eq!(steps[0].affected().deleted(), 1);
    assert_eq!(steps[0].affected().updated(), 1);
    assert_eq!(steps[0].affected().inserted(), 1);
    let result = states
        .iter()
        .find(|state| state.relation() == "target")
        .unwrap();
    assert_eq!(
        result.rows(),
        &[
            row(Some(2), Some(20)),
            row(Some(4), Some(4)),
            row(Some(3), Some(30))
        ]
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(&format!(
        "CREATE TABLE target(id INTEGER,score INTEGER);
        CREATE TABLE source(id INTEGER,score INTEGER);
        INSERT INTO target VALUES (1,2),(2,3),(4,4);
        INSERT INTO source VALUES (1,-5),(2,20),(3,30);
        {sql}"
    ))
    .unwrap();
    let observed: Vec<(i64, i64)> = {
        let mut stmt = db
            .prepare("SELECT id,score FROM target ORDER BY id")
            .unwrap();
        let values = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        values.map(|x| x.unwrap()).collect()
    };
    assert_eq!(observed, vec![(2, 20), (3, 30), (4, 4)]);
}

#[test]
fn duplicated_merge_match_is_residual_not_a_false_success() {
    let b = bundle(
        &["MERGE INTO target USING source ON target.id = source.id
         WHEN MATCHED THEN UPDATE SET score = source.score"],
        "duckdb",
    );
    let plan = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[(Some(1), Some(2))]),
            state("source", &[(Some(1), Some(4)), (Some(1), Some(5))]),
        ],
    );
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn missing_schema_unsupported_ddl_and_incomplete_actions_fail_closed() {
    let b = bundle(
        &["CREATE TABLE target2 AS SELECT id,score FROM source"],
        "duckdb",
    );
    let plan = physical_ordered_write_state_plan(&b, &[state("source", &[(Some(1), Some(2))])]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );

    let b = bundle(&["UPDATE target SET score = score + 1"], "generic");
    let plan = physical_ordered_write_state_plan(&b, &[]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );

    let b = bundle(&["UPDATE target SET score = score / 0"], "generic");
    let plan = physical_ordered_write_state_plan(&b, &[state("target", &[(Some(1), Some(2))])]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn initial_population_is_unique_and_complete() {
    let b = bundle(&["DELETE FROM target"], "generic");
    let target = state("target", &[(Some(1), Some(2))]);
    let duplicate = physical_ordered_write_state_plan(&b, &[target.clone(), target]);
    assert!(matches!(
        duplicate,
        PhysicalWriteStatePlan::Impossible { .. }
    ));
    let plan = physical_ordered_write_state_plan(
        &b,
        &[state("target", &[(Some(1), Some(2)), (Some(1), Some(2))])],
    );
    let (steps, states) = outcome(&plan);
    assert_eq!(steps[0].affected().deleted(), 2);
    assert!(states[0].rows().is_empty());
}

#[test]
fn create_table_from_complete_source_and_reject_existing_target() {
    for dialect in ["duckdb", "postgresql", "snowflake"] {
        let b = bundle(
            &["CREATE TABLE target AS SELECT id, score FROM source"],
            dialect,
        );
        let plan = physical_ordered_write_state_plan(
            &b,
            &[state("source", &[(Some(1), Some(4)), (Some(2), None)])],
        );
        let (steps, states) = outcome(&plan);
        assert_eq!(steps.len(), 1, "{dialect}");
        assert_eq!(steps[0].kind(), WriteKind::Definition);
        assert_eq!(steps[0].affected().inserted(), 2);
        assert_eq!(
            states
                .iter()
                .find(|s| s.relation() == "target")
                .unwrap()
                .rows(),
            &[row(Some(1), Some(4)), row(Some(2), None)]
        );
        let impossible = physical_ordered_write_state_plan(
            &b,
            &[
                state("source", &[(Some(1), Some(4))]),
                state("target", &[(Some(3), Some(3))]),
            ],
        );
        assert!(
            matches!(impossible, PhysicalWriteStatePlan::Impossible { .. }),
            "{impossible:?}"
        );
    }

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER,score INTEGER);
        INSERT INTO source VALUES (1,4),(2,NULL);
        CREATE TABLE target AS SELECT id,score FROM source;",
    )
    .unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM target", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn merge_source_and_target_aliases_share_one_physical_snapshot() {
    let sql = "MERGE INTO target AS t USING source AS s ON t.id=s.id
         WHEN MATCHED THEN UPDATE SET score=s.score
         WHEN NOT MATCHED THEN INSERT (id,score) VALUES (s.id,s.score)";
    let b = bundle(&[sql], "snowflake");
    let plan = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[(Some(1), Some(0))]),
            state("source", &[(Some(1), Some(9)), (Some(2), Some(20))]),
        ],
    );
    let (steps, states) = outcome(&plan);
    assert_eq!(steps[0].affected().updated(), 1);
    assert_eq!(steps[0].affected().inserted(), 1);
    let target = states.iter().find(|s| s.relation() == "target").unwrap();
    assert_eq!(
        target.rows(),
        &[row(Some(1), Some(9)), row(Some(2), Some(20))]
    );
}

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "test-enforced").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

#[test]
fn enforced_key_and_null_constraints_are_checked_on_entire_post_state() {
    let mut b = bundle(
        &["INSERT INTO target (id,score) SELECT id,score FROM source"],
        "duckdb",
    );
    b.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![
            RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap(),
            RelationConstraint::not_null("score", evidence()).unwrap(),
        ],
    )
    .unwrap()]);
    let duplicate = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[]),
            state("source", &[(Some(1), Some(5)), (Some(1), Some(8))]),
        ],
    );
    assert!(
        matches!(duplicate, PhysicalWriteStatePlan::Impossible { .. }),
        "{duplicate:?}"
    );
    let null = physical_ordered_write_state_plan(
        &b,
        &[state("target", &[]), state("source", &[(Some(1), None)])],
    );
    assert!(
        matches!(null, PhysicalWriteStatePlan::Impossible { .. }),
        "{null:?}"
    );
    let valid = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[]),
            state("source", &[(Some(1), Some(5)), (Some(2), Some(8))]),
        ],
    );
    assert!(
        matches!(valid, PhysicalWriteStatePlan::Feasible { .. }),
        "{valid:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER NOT NULL);
        CREATE TABLE source(id INTEGER, score INTEGER);
        INSERT INTO source VALUES (1,5),(1,8);",
    )
    .unwrap();
    assert!(db
        .execute_batch("INSERT INTO target SELECT id,score FROM source;")
        .is_err());
}

#[test]
fn declared_foreign_key_must_have_entire_referenced_population() {
    let mut b = bundle(
        &["INSERT INTO target (id,score) SELECT id,score FROM source"],
        "duckdb",
    );
    b.enrich_relation_constraints(&[
        RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::foreign_key(
                vec!["id".to_string()],
                "source",
                vec!["id".to_string()],
                evidence(),
            )
            .unwrap()],
        )
        .unwrap(),
        RelationConstraintSet::new(
            "source",
            vec![RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap()],
        )
        .unwrap(),
    ]);
    let valid = physical_ordered_write_state_plan(
        &b,
        &[state("target", &[]), state("source", &[(Some(1), Some(9))])],
    );
    assert!(
        matches!(valid, PhysicalWriteStatePlan::Feasible { .. }),
        "{valid:?}"
    );
    let reference_missing = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[(Some(12), Some(1))]),
            state("source", &[(Some(1), Some(9))]),
        ],
    );
    assert!(
        matches!(reference_missing, PhysicalWriteStatePlan::Residual { .. }),
        "{reference_missing:?}"
    );
}

#[test]
fn unsupported_alter_and_create_view_do_not_masquerade_as_physical_tables() {
    let b = bundle(
        &["CREATE VIEW target AS SELECT id,score FROM source"],
        "duckdb",
    );
    let view = physical_ordered_write_state_plan(&b, &[state("source", &[(Some(1), Some(5))])]);
    assert!(
        matches!(view, PhysicalWriteStatePlan::Residual { .. }),
        "{view:?}"
    );
    let b = bundle(&["ALTER TABLE target ADD COLUMN x INTEGER"], "duckdb");
    let alter = physical_ordered_write_state_plan(&b, &[state("target", &[])]);
    assert!(
        matches!(alter, PhysicalWriteStatePlan::Residual { .. }),
        "{alter:?}"
    );
}

#[test]
fn replace_then_update_then_zero_delete_carries_exact_prior_state() {
    let statements = [
        "CREATE OR REPLACE TABLE target AS SELECT id,score FROM source WHERE id > 1",
        "UPDATE target SET score=score+1 WHERE id=2",
        "DELETE FROM target WHERE id=999",
    ];
    let b = bundle(&statements, "snowflake");
    let plan = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[(Some(7), Some(70))]),
            state(
                "source",
                &[(Some(1), Some(-8)), (Some(2), Some(4)), (Some(3), Some(8))],
            ),
        ],
    );
    let (steps, states) = outcome(&plan);
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].affected().deleted(), 1);
    assert_eq!(steps[0].affected().inserted(), 2);
    assert_eq!(steps[1].before().rows(), steps[0].after().rows());
    assert_eq!(steps[1].affected().updated(), 1);
    assert_eq!(steps[2].affected().deleted(), 0);
    assert_eq!(steps[2].after().rows(), steps[1].after().rows());
    assert_eq!(
        states
            .iter()
            .find(|s| s.relation() == "target")
            .unwrap()
            .rows(),
        &[row(Some(2), Some(5)), row(Some(3), Some(8))],
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER,score INTEGER);
        CREATE TABLE target(id INTEGER,score INTEGER);
        INSERT INTO target VALUES (7,70);
        INSERT INTO source VALUES (1,-8),(2,4),(3,8);
        CREATE OR REPLACE TABLE target AS SELECT id,score FROM source WHERE id>1;
        UPDATE target SET score=score+1 WHERE id=2;
        DELETE FROM target WHERE id=999;",
    )
    .unwrap();
    let actual: Vec<(i64, i64)> = {
        let mut stmt = db
            .prepare("SELECT id,score FROM target ORDER BY id")
            .unwrap();
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        rows.map(|r| r.unwrap()).collect()
    };
    assert_eq!(actual, vec![(2, 5), (3, 8)]);
}

#[test]
fn complete_physical_column_domains_preserve_nulls_duplicates_and_empty_bags() {
    let b = bundle(&["DELETE FROM target WHERE score < 0"], "duckdb");
    let schema = b
        .source_schemas()
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    let prior = state(
        "target",
        &[
            (Some(1), Some(10)),
            (Some(1), Some(10)),
            (Some(2), None),
            (Some(3), Some(-5)),
        ],
    );
    let domains = prior.exact_domains(schema).unwrap();
    let score = domains.iter().find(|d| d.column() == "score").unwrap();
    assert_eq!(
        score.values(),
        &[
            (LiteralValue::Number("10".to_string()), 2),
            (LiteralValue::Null, 1),
            (LiteralValue::Number("-5".to_string()), 1),
        ]
    );
    let plan = physical_ordered_write_state_plan(&b, &[prior]);
    let (transitions, _) = outcome(&plan);
    let post = transitions[0].after().exact_domains(schema).unwrap();
    assert_eq!(
        post.iter()
            .find(|d| d.column() == "score")
            .unwrap()
            .values(),
        &[
            (LiteralValue::Number("10".to_string()), 2),
            (LiteralValue::Null, 1)
        ]
    );
    let empty = state("target", &[]);
    let domains = empty.exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 2);
    assert!(domains.iter().all(|domain| domain.values().is_empty()));
}

#[test]
fn one_input_sql_script_uses_statement_indices_and_shared_prior_state() {
    let b = bundle(
        &["DELETE FROM target WHERE id = 9;
           UPDATE target SET score=score+1 WHERE id=1;
           INSERT INTO target (id,score) SELECT id,score FROM source WHERE id=3;
           DELETE FROM target WHERE id < 0"],
        "duckdb",
    );
    let plan = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[(Some(1), Some(2)), (Some(9), Some(4))]),
            state("source", &[(Some(3), Some(5)), (Some(-1), Some(9))]),
        ],
    );
    let (steps, states) = outcome(&plan);
    assert_eq!(steps.len(), 4);
    assert_eq!(steps[0].affected().deleted(), 1);
    assert_eq!(steps[1].affected().updated(), 1);
    assert_eq!(steps[2].affected().inserted(), 1);
    assert_eq!(steps[3].affected().deleted(), 0);
    for pair in steps.windows(2) {
        assert_eq!(pair[0].after(), pair[1].before());
    }
    assert_eq!(
        states
            .iter()
            .find(|s| s.relation() == "target")
            .unwrap()
            .rows(),
        &[row(Some(1), Some(3)), row(Some(3), Some(5))],
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER,score INTEGER);
        CREATE TABLE source(id INTEGER,score INTEGER);
        INSERT INTO target VALUES (1,2),(9,4);
        INSERT INTO source VALUES (3,5),(-1,9);
        DELETE FROM target WHERE id=9;
        UPDATE target SET score=score+1 WHERE id=1;
        INSERT INTO target (id,score) SELECT id,score FROM source WHERE id=3;
        DELETE FROM target WHERE id<0;",
    )
    .unwrap();
    let expected: Vec<(i64, i64)> = {
        let mut stmt = db
            .prepare("SELECT id,score FROM target ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|v| v.unwrap())
            .collect()
    };
    assert_eq!(expected, vec![(1, 3), (3, 5)]);
}

#[test]
fn nullable_unique_keys_and_missing_fk_reference_key_remain_residual() {
    let mut b = bundle(&["DELETE FROM target WHERE id = 999"], "duckdb");
    b.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::unique_key(vec!["id".to_string()], evidence()).unwrap()],
    )
    .unwrap()]);
    let plan = physical_ordered_write_state_plan(
        &b,
        &[state("target", &[(None, Some(1)), (None, Some(2))])],
    );
    assert!(
        matches!(
            plan,
            PhysicalWriteStatePlan::Residual {
                gap: sql_semantic_protocol::PhysicalWriteStateGap::UnprovedConstraint,
                ..
            }
        ),
        "{plan:?}"
    );

    let mut b = bundle(&["DELETE FROM target WHERE id=999"], "duckdb");
    b.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::foreign_key(
            vec!["id".to_string()],
            "source",
            vec!["id".to_string()],
            evidence(),
        )
        .unwrap()],
    )
    .unwrap()]);
    let plan = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[(Some(1), Some(2))]),
            state("source", &[(Some(1), Some(3))]),
        ],
    );
    assert!(
        matches!(
            plan,
            PhysicalWriteStatePlan::Residual {
                gap: sql_semantic_protocol::PhysicalWriteStateGap::UnprovedConstraint,
                ..
            }
        ),
        "{plan:?}"
    );
}

#[test]
fn a_missing_insert_default_remains_residual_without_materialized_value() {
    let b = bundle(&["INSERT INTO target(id) SELECT id FROM source"], "duckdb");
    let plan = physical_ordered_write_state_plan(
        &b,
        &[state("target", &[]), state("source", &[(Some(1), Some(6))])],
    );
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn omitted_insert_column_uses_exact_canonical_default_not_an_intermediate_write() {
    let mut b = bundle(&["INSERT INTO target(id) SELECT id FROM source"], "duckdb");
    b.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
        .unwrap()
        .with_column_value_sources(vec![ColumnValueSource::new(
            "score",
            ColumnValueKind::Default,
            Some(ConstraintValue::Integer(7)),
            evidence(),
        )
        .unwrap()])]);
    let plan = physical_ordered_write_state_plan(
        &b,
        &[
            state("target", &[]),
            state("source", &[(Some(1), Some(9)), (Some(2), None)]),
        ],
    );
    let (_, states) = outcome(&plan);
    assert_eq!(
        states
            .iter()
            .find(|state| state.relation() == "target")
            .unwrap()
            .rows(),
        &[row(Some(1), Some(7)), row(Some(2), Some(7))]
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER, score INTEGER DEFAULT 7);
         CREATE TABLE source(id INTEGER, score INTEGER);
         INSERT INTO source VALUES (1,9),(2,NULL);
         INSERT INTO target(id) SELECT id FROM source;",
    )
    .unwrap();
    assert_eq!(
        db.query_row("SELECT SUM(score) FROM target", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        14
    );
}

#[test]
fn an_opaque_or_generated_default_must_not_be_guessed() {
    for (kind, value) in [
        (ColumnValueKind::Default, None),
        (ColumnValueKind::Generated, None),
        (ColumnValueKind::Identity, None),
    ] {
        let mut b = bundle(&["INSERT INTO target(id) SELECT id FROM source"], "duckdb");
        b.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
            .unwrap()
            .with_column_value_sources(vec![ColumnValueSource::new(
                "score",
                kind,
                value,
                evidence(),
            )
            .unwrap()])]);
        let plan = physical_ordered_write_state_plan(
            &b,
            &[state("target", &[]), state("source", &[(Some(1), Some(3))])],
        );
        assert!(
            matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
            "{kind:?}: {plan:?}"
        );
    }
}

#[test]
fn direct_sql_ddl_retains_default_column_provenance_and_exact_literal() {
    let b = bundle(
        &["CREATE TABLE target(id INTEGER, score INTEGER DEFAULT 7)"],
        "duckdb",
    );
    let set = b
        .relation_constraints()
        .iter()
        .find(|set| set.relation() == "target")
        .expect("CREATE TABLE should expose column value evidence");
    let [default] = set.column_value_sources() else {
        panic!("expected one source, got {:?}", set.column_value_sources());
    };
    assert_eq!(default.column(), "score");
    assert_eq!(default.kind(), ColumnValueKind::Default);
    assert_eq!(default.value(), Some(&ConstraintValue::Integer(7)));
    assert_eq!(
        default.evidence()[0].provenance().source_kind(),
        ConstraintSourceKind::SqlDdl
    );
}

#[test]
fn cyclic_foreign_keys_need_a_joint_closed_world_proof() {
    let mut b = bundle(&["SELECT id FROM target"], "duckdb");
    b.enrich_relation_constraints(&[
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
            ],
        )
        .unwrap(),
        RelationConstraintSet::new(
            "source",
            vec![
                RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap(),
                RelationConstraint::foreign_key(
                    vec!["id".into()],
                    "target",
                    vec!["id".into()],
                    evidence(),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    ]);
    let both = [
        state("target", &[(Some(1), Some(3))]),
        state("source", &[(Some(1), Some(6))]),
    ];
    let plan = physical_closed_world_schema_plan(&b, &both);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Feasible { .. }),
        "{plan:?}"
    );
    assert!(matches!(
        physical_closed_world_schema_plan(&b, &both[..1]),
        PhysicalWriteStatePlan::Residual { .. }
    ));
    let broken = [
        state("target", &[(Some(1), Some(3))]),
        state("source", &[(Some(2), Some(6))]),
    ];
    assert!(matches!(
        physical_closed_world_schema_plan(&b, &broken),
        PhysicalWriteStatePlan::Residual { .. }
    ));
    let duplicate = [
        state("target", &[(Some(1), Some(3)), (Some(1), Some(8))]),
        state("source", &[(Some(1), Some(6))]),
    ];
    assert!(matches!(
        physical_closed_world_schema_plan(&b, &duplicate),
        PhysicalWriteStatePlan::Impossible { .. }
    ));

    // Independent DuckDB oracle for the same joint FK/PK obligations.
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
         CREATE TABLE source(id INTEGER PRIMARY KEY REFERENCES target(id), score INTEGER);
         INSERT INTO target VALUES (1,3);
         INSERT INTO source VALUES (1,6);",
    )
    .unwrap();
    assert!(db.execute_batch("INSERT INTO source VALUES (2,6)").is_err());
}

#[test]
fn enforced_check_comparisons_prove_complete_physical_populations() {
    for dialect in ["duckdb", "postgresql"] {
        let mut b = bundle(
            &["CREATE TABLE target(id INTEGER, score INTEGER CHECK(score > 0))"],
            dialect,
        );
        let ddl = b
            .relation_constraints()
            .iter()
            .find(|set| set.relation() == "target")
            .unwrap();
        assert!(ddl.constraints().iter().any(|rule| matches!(
            rule,
            RelationConstraint::CheckComparison(item)
                if item.column() == "score" && item.operator() == CheckCompareOperator::Gt
                    && item.value() == &ConstraintValue::Integer(0)
        )));
        assert!(
            matches!(
                physical_closed_world_schema_plan(&b, &[state("target", &[(Some(1), Some(2))])]),
                PhysicalWriteStatePlan::Residual { .. }
            ),
            "DDL enforcement not attested: {dialect}"
        );
        b.enrich_relation_constraints(&[RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::check_comparison(
                "score",
                CheckCompareOperator::Gt,
                ConstraintValue::Integer(0),
                evidence(),
            )
            .unwrap()],
        )
        .unwrap()]);
        for expected in [
            state("target", &[(Some(1), Some(2)), (Some(2), None)]),
            state("target", &[]),
        ] {
            let plan = physical_closed_world_schema_plan(&b, &[expected]);
            assert!(
                matches!(plan, PhysicalWriteStatePlan::Feasible { .. }),
                "{dialect}: {plan:?}"
            );
        }
        let invalid =
            physical_closed_world_schema_plan(&b, &[state("target", &[(Some(1), Some(-1))])]);
        assert!(
            matches!(invalid, PhysicalWriteStatePlan::Impossible { .. }),
            "{dialect}: {invalid:?}"
        );
    }

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target (id INTEGER, score INTEGER CHECK(score > 0));
        INSERT INTO target VALUES (1,2),(2,NULL);",
    )
    .unwrap();
    assert!(db
        .execute_batch("INSERT INTO target VALUES (3,-1)")
        .is_err());
}

#[test]
fn arrow_style_array_and_struct_shape_differential_duckdb() {
    let array = DataType::Array {
        element: Some(Box::new(DataType::SignedInteger { bits: Some(32) })),
        length: Some(2),
    };
    let good = PhysicalTypedValue::Array(vec![
        PhysicalTypedValue::Scalar(LiteralValue::Number("1".into())),
        PhysicalTypedValue::Scalar(LiteralValue::Number("2".into())),
    ]);
    let extra = PhysicalTypedValue::Array(vec![
        PhysicalTypedValue::Scalar(LiteralValue::Number("1".into())),
        PhysicalTypedValue::Scalar(LiteralValue::Number("2".into())),
        PhysicalTypedValue::Scalar(LiteralValue::Number("3".into())),
    ]);
    assert_eq!(
        physical_typed_value_plan(&good, &array),
        PhysicalTypeProof::Feasible
    );
    assert!(matches!(
        physical_typed_value_plan(&extra, &array),
        PhysicalTypeProof::Impossible { .. }
    ));

    let record = DataType::Struct {
        fields: vec![
            DataTypeField::new(Some("a".into()), DataType::SignedInteger { bits: Some(32) }),
            DataTypeField::new(
                Some("b".into()),
                DataType::String {
                    length: None,
                    fixed: false,
                },
            ),
        ],
    };
    assert_eq!(
        physical_typed_value_plan(
            &PhysicalTypedValue::Struct(vec![
                PhysicalTypedValue::Scalar(LiteralValue::Number("3".into())),
                PhysicalTypedValue::Scalar(LiteralValue::Text("ok".into())),
            ]),
            &record,
        ),
        PhysicalTypeProof::Feasible
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE typed (arr INTEGER[2], rec STRUCT(a INTEGER, b VARCHAR));
        INSERT INTO typed VALUES ([1,2], {'a':3,'b':'ok'});",
    )
    .unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM typed", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(db
        .execute_batch("INSERT INTO typed VALUES ([1,2,3], {'a':3,'b':'ok'})")
        .is_err());
}
