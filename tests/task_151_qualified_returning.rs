//! TASK-151 bounded seam: qualified target RETURNING preserves exact physical bags.
//! Other task variants remain pending in docs/task-151-acceptance-freeze.json.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const BEFORE: [(Option<i64>, i64); 4] = [
    (Some(1), 10),
    (Some(2), 20),
    (None, 30),
    (Some(2), 20),
];

fn state(rows: &[(Option<i64>, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        "target",
        rows.iter()
            .map(|(id, score)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".to_string(),
                        id.map_or(LiteralValue::Null, |value| {
                            LiteralValue::Number(value.to_string())
                        }),
                    ),
                    ("score".to_string(), LiteralValue::Number(score.to_string())),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn rows(state: &PhysicalWriteState) -> Vec<(Option<i64>, i64)> {
    let mut rows = state
        .rows()
        .iter()
        .map(|row| {
            let id = match row.values().get("id").unwrap() {
                LiteralValue::Null => None,
                LiteralValue::Number(number) => Some(number.parse::<i64>().unwrap()),
                other => panic!("unexpected integer id: {other:?}"),
            };
            let score = match row.values().get("score").unwrap() {
                LiteralValue::Number(number) => number.parse::<i64>().unwrap(),
                other => panic!("unexpected integer score: {other:?}"),
            };
            (id, score)
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn bundle(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new(
        "target",
        ["id", "score"]
            .into_iter()
            .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline(sql);
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = ConfiguredSqlInput::new("task151", &input, "duckdb", dialect.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap()
}

fn native() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target (id INTEGER, score INTEGER);
         INSERT INTO target VALUES (1, 10), (2, 20), (NULL, 30), (2, 20);",
    )
    .unwrap();
    db
}

fn native_target_rows(db: &Connection) -> Vec<(Option<i64>, i64)> {
    let mut query = db.prepare("SELECT id,score FROM target").unwrap();
    let mut bag = query
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    bag.sort();
    bag
}

#[test]
fn qualified_update_returning_is_exact_after_image_with_duplicates_and_unknown() {
    let sql = "UPDATE target AS t SET score = 40 WHERE t.id <= 2 RETURNING t.id";
    let bundle = bundle(sql);
    let plan = physical_ordered_write_state_plan(&bundle, &[state(&BEFORE)]);
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } = plan else {
        panic!("qualified RETURNING should be exact: {plan:?}");
    };
    let [transition] = transitions.as_slice() else {
        panic!("expected one write transition");
    };
    assert_eq!(transition.before(), &state(&BEFORE));
    assert_eq!(transition.affected().updated(), 3);
    assert_eq!(transition.affected_target_ordinals(), &[0, 1, 3]);
    assert_eq!(final_states, vec![transition.after().clone()]);
    assert_eq!(
        rows(transition.after()),
        rows(&state(&[(Some(1), 40), (Some(2), 40), (None, 30), (Some(2), 40)])),
    );
    let returned = transition.returning().expect("RETURNING is present");
    let returned_ids = returned.rows().iter().map(|row| {
        match row.values().get("id").unwrap() {
            LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
            other => panic!("unexpected returned id: {other:?}"),
        }
    }).collect::<Vec<_>>();
    assert_eq!(returned_ids, vec![1, 2, 2]);
    let schema = &bundle.source_schemas()[0];
    let domains = transition.returning_exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 1);
    assert_eq!(domains[0].column(), "id");
    assert_eq!(
        domains[0].values(),
        &[
            (LiteralValue::Number("1".to_string()), 1),
            (LiteralValue::Number("2".to_string()), 2),
        ]
    );
    let native = native();
    let mut statement = native.prepare(sql).unwrap();
    let mut native_returning = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    drop(statement);
    native_returning.sort();
    assert_eq!(native_returning, vec![1, 2, 2]);
    assert_eq!(rows(transition.after()), native_target_rows(&native));
}

#[test]
fn qualified_delete_returning_is_exact_before_image_for_each_duplicate() {
    let sql = "DELETE FROM target AS t WHERE t.id = 2 RETURNING t.id, t.score";
    let bundle = bundle(sql);
    let plan = physical_ordered_write_state_plan(&bundle, &[state(&BEFORE)]);
    let PhysicalWriteStatePlan::Feasible { transitions, final_states } = plan else {
        panic!("qualified DELETE RETURNING should be exact: {plan:?}");
    };
    let [transition] = transitions.as_slice() else {
        panic!("expected one write transition");
    };
    assert_eq!(transition.affected().deleted(), 2);
    assert_eq!(transition.affected_target_ordinals(), &[1, 3]);
    assert_eq!(final_states, vec![transition.after().clone()]);
    assert_eq!(rows(transition.after()), rows(&state(&[(Some(1), 10), (None, 30)])));
    let returned = transition.returning().expect("RETURNING is present");
    let returned_rows = returned.rows().iter().map(|row| {
        let id = match row.values().get("id").unwrap() {
            LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
            other => panic!("unexpected returned id: {other:?}"),
        };
        let score = match row.values().get("score").unwrap() {
            LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
            other => panic!("unexpected returned score: {other:?}"),
        };
        (id, score)
    }).collect::<Vec<_>>();
    assert_eq!(returned_rows, vec![(2, 20), (2, 20)]);
    let domains = transition.returning_exact_domains(&bundle.source_schemas()[0]).unwrap();
    assert_eq!(domains.len(), 2);
    assert_eq!(domains[0].values(), &[(LiteralValue::Number("2".to_string()), 2)]);
    assert_eq!(domains[1].values(), &[(LiteralValue::Number("20".to_string()), 2)]);
    let native = native();
    let mut statement = native.prepare(sql).unwrap();
    let native_returning = statement
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    drop(statement);
    assert_eq!(native_returning, vec![(2, 20), (2, 20)]);
    assert_eq!(rows(transition.after()), native_target_rows(&native));
}

#[test]
fn unbound_or_wildcard_returning_does_not_fabricate_target_columns() {
    for sql in [
        "UPDATE target AS t SET score = 40 RETURNING source.id",
        "DELETE FROM target WHERE id = 2 RETURNING *",
    ] {
        let plan = physical_ordered_write_state_plan(&bundle(sql), &[state(&BEFORE)]);
        assert!(matches!(plan, PhysicalWriteStatePlan::Residual { .. }), "{sql}: {plan:?}");
    }
}

#[test]
fn alias_and_unqualified_returning_collision_is_rejected() {
    let sql = "UPDATE target AS t SET score = 40 RETURNING t.id, id";
    let plan = physical_ordered_write_state_plan(&bundle(sql), &[state(&BEFORE)]);
    assert!(matches!(plan, PhysicalWriteStatePlan::Residual { .. }), "{plan:?}");
}
