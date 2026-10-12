//! Closed-world ordered write construction and DuckDB statement oracles.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    AnalysisBundle, ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput, WriteKind,
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
