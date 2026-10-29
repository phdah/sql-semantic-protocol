//! TASK-149: independent complete-bag UPDATE/DELETE execution oracle.
//! Frozen before semantic edits in docs/task-149-acceptance-freeze.json.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ColumnValueKind, ColumnValueSource, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

fn state(values: &[(Option<i64>, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "target",
        values
            .iter()
            .map(|(id, score)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".into(),
                        id.map_or(LiteralValue::Null, |x| LiteralValue::Number(x.to_string())),
                    ),
                    ("score".into(), LiteralValue::Number(score.to_string())),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn bundle(sql: &str, default: bool) -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new(
        "target",
        ["id", "score"]
            .into_iter()
            .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(sql);
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = ConfiguredSqlInput::new("task149", &input, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap();
    if default {
        let evidence = vec![ConstraintEvidence::new(
            ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task149").unwrap(),
            ConstraintEnforcement::Enforced,
        )];
        let source = ColumnValueSource::new(
            "score",
            ColumnValueKind::Default,
            Some(ConstraintValue::Integer(7)),
            evidence,
        )
        .unwrap();
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new("target", vec![])
            .unwrap()
            .with_column_value_sources(vec![source])]);
    }
    bundle
}

fn native_rows(db: &Connection) -> Vec<(Option<i64>, i64)> {
    let mut query = db
        .prepare("SELECT id,score FROM target ORDER BY score,id NULLS LAST")
        .unwrap();
    query
        .query_map([], |r| {
            Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn duckdb() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER, score INTEGER DEFAULT 7);
         INSERT INTO target(id,score) VALUES (1,10),(2,20),(NULL,30),(2,20);",
    )
    .unwrap();
    db
}

fn sorted(mut bag: Vec<(Option<i64>, i64)>) -> Vec<(Option<i64>, i64)> {
    bag.sort();
    bag
}

fn run(
    sql: &str,
    default: bool,
) -> (
    sql_semantic_protocol::PhysicalWriteTransition,
    Vec<(Option<i64>, i64)>,
) {
    let before = [(Some(1), 10), (Some(2), 20), (None, 30), (Some(2), 20)];
    let bundle = bundle(sql, default);
    let plan = physical_ordered_write_state_plan(&bundle, &[state(&before)]);
    let PhysicalWriteStatePlan::Feasible {
        mut transitions,
        final_states,
    } = plan
    else {
        panic!("TASK-149 must construct exact physical state: {plan:?}");
    };
    assert_eq!(transitions.len(), 1);
    let transition = transitions.remove(0);
    assert_eq!(transition.before(), &state(&before));
    assert_eq!(final_states.len(), 1);
    assert_eq!(transition.after(), &final_states[0]);
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    let domains = transition.after().exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 2);
    let id = &domains[0];
    assert_eq!(id.column(), "id");
    assert_eq!(
        id.values().iter().map(|(_, count)| *count).sum::<u64>(),
        transition.after().rows().len() as u64
    );
    let final_rows = transition
        .after()
        .rows()
        .iter()
        .map(|row| {
            let id = match row.values().get("id").unwrap() {
                LiteralValue::Null => None,
                LiteralValue::Number(n) => Some(n.parse::<i64>().unwrap()),
                other => panic!("unexpected id {other:?}"),
            };
            let score = match row.values().get("score").unwrap() {
                LiteralValue::Number(n) => n.parse::<i64>().unwrap(),
                other => panic!("unexpected score {other:?}"),
            };
            (id, score)
        })
        .collect::<Vec<_>>();
    (transition, final_rows)
}

#[test]
fn update_literals_computed_default_and_three_valued_selection() {
    let cases = [
        ("UPDATE target SET score = 40 WHERE id = 1", false, 1),
        ("UPDATE target SET score = score + 5 WHERE id = 2", false, 2),
        ("UPDATE target SET score = DEFAULT WHERE id = 1", true, 1),
        ("UPDATE target SET score = 99 WHERE id = NULL", false, 0),
    ];
    for (sql, default, affected) in cases {
        let (transition, expected) = run(sql, default);
        assert_eq!(transition.affected().updated(), affected, "{sql}");
        assert_eq!(transition.affected().deleted(), 0);
        let db = duckdb();
        db.execute_batch(sql).unwrap();
        assert_eq!(sorted(expected), sorted(native_rows(&db)), "{sql}");
    }
}

#[test]
fn delete_preserves_not_true_unknown_duplicates_and_complete_bag() {
    for (sql, count) in [
        ("DELETE FROM target WHERE id = 2", 2),
        ("DELETE FROM target WHERE id = NULL", 0),
        ("DELETE FROM target WHERE id = 99", 0),
    ] {
        let (transition, expected) = run(sql, false);
        assert_eq!(transition.affected().deleted(), count);
        assert_eq!(transition.affected().updated(), 0);
        let db = duckdb();
        db.execute_batch(sql).unwrap();
        assert_eq!(sorted(expected), sorted(native_rows(&db)), "{sql}");
    }
}

#[test]
fn mutation_returning_is_exact_before_delete_and_after_update() {
    for (sql, expected_returning) in [
        (
            "UPDATE target SET score = 40 WHERE id = 1 RETURNING id,score",
            vec![(Some(1), 40)],
        ),
        (
            "DELETE FROM target WHERE id = 2 RETURNING id,score",
            vec![(Some(2), 20), (Some(2), 20)],
        ),
    ] {
        let (transition, expected_after) = run(sql, false);
        let returned = transition
            .returning()
            .expect("supported RETURNING must be present");
        let actual = returned
            .rows()
            .iter()
            .map(|r| {
                let id = match r.values().get("id").unwrap() {
                    LiteralValue::Null => None,
                    LiteralValue::Number(value) => Some(value.parse::<i64>().unwrap()),
                    _ => panic!("not an INTEGER id"),
                };
                let score = match r.values().get("score").unwrap() {
                    LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
                    _ => panic!("not an INTEGER score"),
                };
                (id, score)
            })
            .collect::<Vec<_>>();
        assert_eq!(sorted(actual), sorted(expected_returning.clone()));
        let schema = bundle(sql, false)
            .source_schemas()
            .iter()
            .find(|s| s.relation() == "target")
            .unwrap()
            .clone();
        let exact = transition.returning_exact_domains(&schema).unwrap();
        assert_eq!(exact.len(), 2);
        let db = duckdb();
        let mut statement = db.prepare(sql).unwrap();
        let native = statement
            .query_map([], |r| {
                Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        drop(statement);
        assert_eq!(sorted(native), sorted(expected_returning));
        assert_eq!(sorted(expected_after), sorted(native_rows(&db)));
    }
}

#[test]
fn unattested_default_cannot_be_constructed() {
    let sql = "UPDATE target SET score = DEFAULT WHERE id = 1";
    let before = state(&[(Some(1), 10)]);
    let plan = physical_ordered_write_state_plan(&bundle(sql, false), &[before]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn mutation_enforced_key_collision_is_impossible_and_native_engine_rejects() {
    let sql = "UPDATE target SET id = 2 WHERE id = 1";
    let mut b = bundle(sql, false);
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task149-pk").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    b.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".into()], evidence).unwrap()],
    )
    .unwrap()]);
    let initial = state(&[(Some(1), 10), (Some(2), 20)]);
    let plan = physical_ordered_write_state_plan(&b, &[initial]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "{plan:?}"
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
         INSERT INTO target VALUES (1,10),(2,20);",
    )
    .unwrap();
    assert!(db.execute_batch(sql).is_err(), "duplicate PK must fail");
    assert_eq!(native_rows(&db), vec![(Some(1), 10), (Some(2), 20)]);
}

#[test]
fn zero_selected_rows_produce_empty_but_typed_returning_population() {
    let sql = "DELETE FROM target WHERE id = NULL RETURNING id,score";
    let (transition, after) = run(sql, false);
    assert_eq!(transition.affected().deleted(), 0);
    assert!(transition.returning().unwrap().rows().is_empty());
    let bundle = bundle(sql, false);
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    let domains = transition.returning_exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 2);
    assert!(domains.iter().all(|domain| domain.values().is_empty()));
    let db = duckdb();
    let native = {
        let mut statement = db.prepare(sql).unwrap();
        statement
            .query_map([], |r| r.get::<_, Option<i64>>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert!(native.is_empty());
    assert_eq!(sorted(after), sorted(native_rows(&db)));
}
