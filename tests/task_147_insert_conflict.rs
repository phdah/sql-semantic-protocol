//! TASK-147: complete targeted INSERT conflict and RETURNING physical oracle.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    AnalysisBundle, ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput,
};

const SQL: &str = "INSERT INTO target(id,score)
    SELECT id,score FROM source WHERE TRUE
    ON CONFLICT(id) DO NOTHING RETURNING id";

fn row(id: Option<i64>, score: i64) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        (
            "id".to_string(),
            id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
        ),
        ("score".to_string(), LiteralValue::Number(score.to_string())),
    ]))
    .unwrap()
}

fn state(relation: &str, data: &[(Option<i64>, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        data.iter().map(|(id, score)| row(*id, *score)).collect(),
    )
    .unwrap()
}

fn bundle(sql: &str, enforced: bool) -> AnalysisBundle {
    let schemas = ["target", "source"].map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "score"]
                .into_iter()
                .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap())
                .collect(),
        )
        .unwrap()
    });
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let configured = ConfiguredSqlInput::new("task147", &input, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap();
    if enforced {
        let evidence = vec![ConstraintEvidence::new(
            ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task147").unwrap(),
            ConstraintEnforcement::Enforced,
        )];
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::primary_key(vec!["id".into()], evidence).unwrap()],
        )
        .unwrap()]);
    }
    bundle
}

#[test]
fn targeted_conflict_preserves_existing_row_and_returns_only_inserted_id() {
    let source = state("source", &[(Some(1), 20), (Some(2), 30)]);
    let before = state("target", &[(Some(1), 10)]);
    let plan =
        physical_ordered_write_state_plan(&bundle(SQL, true), &[source.clone(), before.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("expected exact conflict handling: {plan:?}");
    };
    assert_eq!(transitions.len(), 1);
    let transition = &transitions[0];
    assert_eq!(transition.before(), &before);
    assert_eq!(
        transition.after().rows(),
        state("target", &[(Some(1), 10), (Some(2), 30)]).rows()
    );
    assert_eq!(transition.affected().inserted(), 1);
    assert_eq!(transition.affected().updated(), 0);
    assert_eq!(transition.affected().deleted(), 0);
    let returned = transition.returning().expect("normalized RETURNING clause");
    assert_eq!(returned.rows().len(), 1);
    assert_eq!(
        returned.rows()[0].values(),
        &BTreeMap::from([("id".to_string(), LiteralValue::Number("2".into()))])
    );
    assert_eq!(
        final_states
            .iter()
            .find(|state| state.relation() == "source"),
        Some(&source),
        "source is immutable across the write"
    );
    assert_eq!(final_states.len(), 2);

    // The independent DuckDB oracle exercises complete source, prestate,
    // poststate and actual RETURNING output rather than just row counts.
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER,score INTEGER);
         CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);
         INSERT INTO target VALUES (1,10);
         INSERT INTO source VALUES (1,20),(2,30);",
    )
    .unwrap();
    let mut statement = db.prepare(SQL).unwrap();
    let observed_returned = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(observed_returned, vec![2]);
    drop(statement);
    let mut query = db
        .prepare("SELECT id,score FROM target ORDER BY id")
        .unwrap();
    let observed = query
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(observed, vec![(1, 10), (2, 30)]);
}

#[test]
fn same_statement_duplicate_keys_are_rejected_by_full_target_bag() {
    let data = [
        state("source", &[(Some(1), 20), (Some(1), 20)]),
        state("target", &[]),
    ];
    let plan = physical_ordered_write_state_plan(&bundle(SQL, true), &data);
    let PhysicalWriteStatePlan::Feasible { transitions, .. } = plan else {
        panic!("conflicts from earlier inserted rows must be suppressed: {plan:?}");
    };
    assert_eq!(transitions[0].affected().inserted(), 1);
    assert_eq!(
        transitions[0].after().rows(),
        state("target", &[(Some(1), 20)]).rows()
    );
    assert_eq!(transitions[0].returning().unwrap().rows().len(), 1);
}

#[test]
fn unordered_conflicting_source_payloads_are_residual() {
    let data = [
        state("source", &[(Some(1), 20), (Some(1), 30)]),
        state("target", &[]),
    ];
    let plan = physical_ordered_write_state_plan(&bundle(SQL, true), &data);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn unverified_conflict_target_is_residual_even_when_target_is_empty() {
    let data = [state("source", &[(Some(1), 20)]), state("target", &[])];
    let plan = physical_ordered_write_state_plan(&bundle(SQL, false), &data);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn nullable_primary_key_is_impossible_not_a_suppressed_conflict() {
    let data = [state("source", &[(None, 20)]), state("target", &[])];
    let plan = physical_ordered_write_state_plan(&bundle(SQL, true), &data);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "{plan:?}"
    );
}

#[test]
fn unproved_vendor_and_upsert_actions_are_explicitly_unsupported() {
    for sql in [
        "INSERT INTO target(id,score) VALUES (1,2) ON CONFLICT(id) DO UPDATE SET score=3",
        "INSERT INTO target(id,score) VALUES (1,2) ON CONFLICT DO NOTHING",
        "INSERT INTO target(id,score) VALUES (1,2) RETURNING id+1",
    ] {
        let bundle = bundle(sql, true);
        assert!(
            matches!(
                physical_ordered_write_state_plan(&bundle, &[state("target", &[])]),
                PhysicalWriteStatePlan::Residual { .. }
            ),
            "{sql}"
        );
    }
}
