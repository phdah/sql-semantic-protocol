//! TASK-150: complete-bag joined UPDATE/DELETE, with an independent DuckDB oracle.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

fn state(relation: &str, rows: &[(Option<i64>, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter()
            .map(|(id, score)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".to_string(),
                        id.map_or(LiteralValue::Null, |id| {
                            LiteralValue::Number(id.to_string())
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

fn analyze(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["target", "source"]
        .into_iter()
        .map(|name| {
            RelationSchema::new(
                name,
                ["id", "score"]
                    .into_iter()
                    .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap())
                    .collect(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let input = SqlInput::inline(sql);
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = ConfiguredSqlInput::new("task150", &input, "duckdb", dialect.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap()
}

fn rows(state: &PhysicalWriteState) -> Vec<(Option<i64>, i64)> {
    let mut rows = state
        .rows()
        .iter()
        .map(|row| {
            let id = match row.values().get("id").unwrap() {
                LiteralValue::Null => None,
                LiteralValue::Number(n) => Some(n.parse::<i64>().unwrap()),
                other => panic!("unexpected id: {other:?}"),
            };
            let score = match row.values().get("score").unwrap() {
                LiteralValue::Number(n) => n.parse::<i64>().unwrap(),
                other => panic!("unexpected score: {other:?}"),
            };
            (id, score)
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn native(
    before: &[(Option<i64>, i64)],
    source: &[(Option<i64>, i64)],
    sql: &str,
) -> Vec<(Option<i64>, i64)> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE target(id INTEGER, score INTEGER); CREATE TABLE source(id INTEGER, score INTEGER);").unwrap();
    for (relation, rows) in [("target", before), ("source", source)] {
        for (id, score) in rows {
            let sql = format!(
                "INSERT INTO {relation}(id, score) VALUES ({}, {score})",
                id.map_or("NULL".to_string(), |id| id.to_string())
            );
            db.execute_batch(&sql).unwrap();
        }
    }
    db.execute_batch(sql).unwrap();
    let mut stmt = db.prepare("SELECT id, score FROM target").unwrap();
    let mut result = stmt
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn exact(
    before: &[(Option<i64>, i64)],
    source: &[(Option<i64>, i64)],
    sql: &str,
    updated: u64,
    deleted: u64,
) {
    let bundle = analyze(sql);
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[state("target", before), state("source", source)],
    );
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("joined write must be exact: {plan:?}");
    };
    assert_eq!(transitions.len(), 1);
    let transition = &transitions[0];
    assert_eq!(rows(transition.before()), rows(&state("target", before)));
    assert_eq!(transition.affected().updated(), updated);
    assert_eq!(transition.affected().deleted(), deleted);
    let final_target = final_states
        .iter()
        .find(|state| state.relation() == "target")
        .unwrap();
    let final_source = final_states
        .iter()
        .find(|state| state.relation() == "source")
        .unwrap();
    assert_eq!(rows(transition.after()), rows(final_target));
    assert_eq!(rows(final_source), rows(&state("source", source)));
    assert_eq!(rows(final_target), native(before, source, sql));
    let target_schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "target")
        .unwrap();
    let domains = final_target.exact_domains(target_schema).unwrap();
    assert_eq!(domains.len(), 2);
    for domain in domains {
        assert_eq!(
            domain.values().iter().map(|(_, count)| *count).sum::<u64>(),
            final_target.rows().len() as u64
        );
    }
}

#[test]
fn unique_update_from_preserves_unmatched_and_null_target_rows() {
    exact(
        &[(Some(1), 10), (Some(2), 20), (None, 30)],
        &[(Some(1), 40)],
        "UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id",
        1,
        0,
    );
}

#[test]
fn delete_using_uses_complete_source_and_preserves_not_true_rows() {
    exact(
        &[(Some(1), 10), (Some(2), 20), (None, 30)],
        &[(Some(1), 40), (None, 50)],
        "DELETE FROM target AS t USING source AS s WHERE t.id = s.id",
        0,
        1,
    );
}

#[test]
fn empty_source_does_not_delete_any_target_rows() {
    exact(
        &[(Some(1), 10), (Some(2), 20), (None, 30)],
        &[],
        "DELETE FROM target AS t USING source AS s",
        0,
        0,
    );
}

#[test]
fn duplicate_update_matches_fail_closed_instead_of_choosing_an_arbitrary_row() {
    let sql = "UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id";
    let bundle = analyze(sql);
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[
            state("target", &[(Some(1), 10)]),
            state("source", &[(Some(1), 40), (Some(1), 50)]),
        ],
    );
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn missing_source_population_is_residual_not_an_empty_source() {
    let bundle = analyze("DELETE FROM target AS t USING source AS s WHERE t.id = s.id");
    let plan = physical_ordered_write_state_plan(&bundle, &[state("target", &[(Some(1), 10)])]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn duplicate_delete_using_matches_remove_each_target_occurrence_once() {
    exact(
        &[(Some(1), 10), (Some(1), 10), (Some(2), 20), (None, 30)],
        &[(Some(1), 40), (Some(1), 50)],
        "DELETE FROM target AS t USING source AS s WHERE t.id = s.id",
        0,
        2,
    );
}

#[test]
fn joined_update_computed_value_uses_the_matching_source_row() {
    exact(
        &[(Some(1), 10), (Some(2), 20), (None, 30)],
        &[(Some(1), 40), (Some(3), 50)],
        "UPDATE target AS t SET score = s.score + 5 FROM source AS s WHERE t.id = s.id",
        1,
        0,
    );
}

#[test]
fn joined_null_equality_never_selects_a_target() {
    exact(
        &[(Some(1), 10), (None, 30)],
        &[(None, 40)],
        "DELETE FROM target AS t USING source AS s WHERE t.id = s.id",
        0,
        0,
    );
}
