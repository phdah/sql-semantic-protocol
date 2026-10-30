//! TASK-152: task-owned complete before/after bags for ordered INSERT, UPDATE, DELETE.
//! Expectations are frozen independently of the canonical physical interpreter.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    CheckCompareOperator, ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStateGap, PhysicalWriteStatePlan, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-152-acceptance-freeze.json")).unwrap()
}

fn row(value: &Value) -> PhysicalWriteRow {
    let object = value.as_object().unwrap();
    PhysicalWriteRow::new(
        object
            .iter()
            .map(|(column, value)| {
                (
                    column.to_string(),
                    value.as_i64().map_or(LiteralValue::Null, |number| {
                        LiteralValue::Number(number.to_string())
                    }),
                )
            })
            .collect(),
    )
    .unwrap()
}

fn state(relation: &str, rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(relation, rows.as_array().unwrap().iter().map(row).collect()).unwrap()
}

fn enforced() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-152").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn catalog() -> RelationCatalog {
    let parent = RelationSchema::new(
        "parent",
        vec![SchemaColumn::from_sql_type("id", "INTEGER", "duckdb").unwrap()],
    )
    .unwrap();
    let target = RelationSchema::new(
        "target",
        ["id", "parent_id", "score"]
            .into_iter()
            .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", "duckdb").unwrap())
            .collect(),
    )
    .unwrap();
    RelationCatalog::from_schemas(&[target, parent]).unwrap()
}

fn analysis(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let queries = case["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| SqlInput::inline(step["sql"].as_str().unwrap()))
        .collect::<Vec<_>>();
    let ids = (0..queries.len())
        .map(|i| format!("task152-{i}"))
        .collect::<Vec<_>>();
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = queries
        .iter()
        .zip(&ids)
        .map(|(query, id)| ConfiguredSqlInput::new(id, query, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&inputs, &catalog()).unwrap();
    bundle.enrich_relation_constraints(&[
        RelationConstraintSet::new(
            "parent",
            vec![RelationConstraint::primary_key(vec!["id".to_string()], enforced()).unwrap()],
        )
        .unwrap(),
        RelationConstraintSet::new(
            "target",
            vec![
                RelationConstraint::primary_key(vec!["id".to_string()], enforced()).unwrap(),
                RelationConstraint::foreign_key(
                    vec!["parent_id".to_string()],
                    "parent",
                    vec!["id".to_string()],
                    enforced(),
                )
                .unwrap(),
                RelationConstraint::check_comparison(
                    "score",
                    CheckCompareOperator::Gte,
                    ConstraintValue::Integer(0),
                    enforced(),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    ]);
    bundle
}

fn initial(case: &Value) -> Vec<PhysicalWriteState> {
    let mut input = vec![state("target", &case["target_before"])];
    if case["assumed_parent"].is_array() {
        input.push(state("parent", &case["assumed_parent"]));
    }
    input
}

fn read_native(db: &Connection) -> PhysicalWriteState {
    let mut stmt = db
        .prepare("SELECT id, parent_id, score FROM target ORDER BY rowid")
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .map(|(id, parent_id, score)| {
            PhysicalWriteRow::new(BTreeMap::from([
                ("id".into(), LiteralValue::Number(id.to_string())),
                (
                    "parent_id".into(),
                    parent_id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                ),
                ("score".into(), LiteralValue::Number(score.to_string())),
            ]))
            .unwrap()
        })
        .collect::<Vec<_>>();
    PhysicalWriteState::new("target", rows).unwrap()
}

fn native(case: &Value) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE parent(id INTEGER PRIMARY KEY);
         CREATE TABLE target(
             id INTEGER PRIMARY KEY,
             parent_id INTEGER REFERENCES parent(id),
             score INTEGER CHECK(score >= 0)
         );
         INSERT INTO parent VALUES (7);",
    )
    .unwrap();
    for row in case["target_before"].as_array().unwrap() {
        db.execute(
            "INSERT INTO target(id,parent_id,score) VALUES (?1,?2,?3)",
            params![
                row["id"].as_i64().unwrap(),
                row["parent_id"].as_i64(),
                row["score"].as_i64().unwrap()
            ],
        )
        .unwrap();
    }
    db
}

fn selected(db: &Connection, predicate: &str, not_true: bool) -> Vec<usize> {
    let condition = if not_true { "IS NOT TRUE" } else { "IS TRUE" };
    let sql = format!(
        "SELECT ordinal FROM (
           SELECT row_number() OVER (ORDER BY rowid) - 1 AS ordinal, id FROM target
         ) numbered WHERE ({predicate}) {condition} ORDER BY ordinal"
    );
    let mut stmt = db.prepare(&sql).unwrap();
    stmt.query_map([], |r| r.get::<_, i64>(0))
        .unwrap()
        .map(Result::unwrap)
        .map(|ordinal| usize::try_from(ordinal).unwrap())
        .collect()
}

fn assert_native_truth(db: &Connection, step: &Value) {
    let sql = step["sql"].as_str().unwrap();
    let Some((_, predicate)) = sql.split_once(" WHERE ") else {
        return;
    };
    let expected = |key: &str| {
        step[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|index| usize::try_from(index.as_u64().unwrap()).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        selected(db, predicate, false),
        expected("selected_prestate_ordinals"),
        "{sql}: independent TRUE target identities"
    );
    assert_eq!(
        selected(db, predicate, true),
        expected("not_true_prestate_ordinals"),
        "{sql}: independent NOT TRUE target identities"
    );
}

fn assert_domains(state: &PhysicalWriteState, schema: &RelationSchema, frozen: &Value) {
    let actual = state.exact_domains(schema).unwrap();
    assert_eq!(actual.len(), 3);
    for column in &actual {
        let expectation = &frozen[column.column()];
        let observed = column
            .values()
            .iter()
            .map(|(value, count)| {
                (
                    match value {
                        LiteralValue::Null => "NULL".to_string(),
                        LiteralValue::Number(n) => n.clone(),
                        other => panic!("unexpected scalar {other:?}"),
                    },
                    *count,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let expected = expectation["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, count)| (key.clone(), count.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(observed, expected, "{} exact frequencies", column.column());
        let observed_numbers = observed
            .iter()
            .filter(|(key, _)| key.as_str() != "NULL")
            .map(|(key, _)| key.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            observed_numbers.iter().min().copied(),
            expectation["lower"].as_i64()
        );
        assert_eq!(
            observed_numbers.iter().max().copied(),
            expectation["upper"].as_i64()
        );
        assert_eq!(
            observed.contains_key("NULL"),
            expectation["nullable"].as_bool().unwrap()
        );
        assert_eq!(
            !observed_numbers.is_empty(),
            expectation["lower_inclusive"].as_bool().unwrap()
        );
        assert_eq!(
            !observed_numbers.is_empty(),
            expectation["upper_inclusive"].as_bool().unwrap()
        );
    }
}

#[test]
fn every_frozen_sequential_write_obeys_complete_native_oracle() {
    let freeze = freeze();
    assert_eq!(freeze["version"], 1);
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 7);
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let bundle = analysis(case);
        let plan = physical_ordered_write_state_plan(&bundle, &initial(case));
        if case["expected_failure"] == "Residual" {
            assert!(
                matches!(
                    plan,
                    PhysicalWriteStatePlan::Residual {
                        gap: PhysicalWriteStateGap::MissingPopulation,
                        ..
                    }
                ),
                "{id}: missing referenced relation must not become empty: {plan:?}"
            );
            continue;
        }
        let db = native(case);
        let parent: i64 = db
            .query_row("SELECT id FROM parent", [], |r| r.get(0))
            .unwrap();
        assert_eq!(parent, 7, "{id}: complete parent-source bag");

        if case["expected_failure"] == "Impossible" {
            assert!(
                matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
                "{id}: required contradiction: {plan:?}"
            );
        } else {
            let PhysicalWriteStatePlan::Feasible {
                transitions,
                final_states,
            } = &plan
            else {
                panic!("{id}: all three writes must be feasible: {plan:?}");
            };
            assert_eq!(transitions.len(), case["steps"].as_array().unwrap().len());
            let schema = bundle
                .source_schemas()
                .iter()
                .find(|schema| schema.relation() == "target")
                .unwrap();
            for (index, transition) in transitions.iter().enumerate() {
                let step = &case["steps"][index];
                assert_eq!(
                    transition.before(),
                    &state("target", &step["before"]),
                    "{id}: before #{index}"
                );
                assert_eq!(
                    transition.after(),
                    &state("target", &step["after"]),
                    "{id}: after #{index}"
                );
                assert_eq!(
                    transition.affected().inserted(),
                    step["affected"]["inserted"].as_u64().unwrap()
                );
                assert_eq!(
                    transition.affected().updated(),
                    step["affected"]["updated"].as_u64().unwrap()
                );
                assert_eq!(
                    transition.affected().deleted(),
                    step["affected"]["deleted"].as_u64().unwrap()
                );
                assert_domains(transition.before(), schema, &step["before_domains"]);
                assert_domains(transition.after(), schema, &step["after_domains"]);
                if index > 0 {
                    assert_eq!(
                        transition.before(),
                        transitions[index - 1].after(),
                        "{id}: preserve prior committed image"
                    );
                }
                if step["sql"].as_str().unwrap().starts_with("UPDATE ")
                    || step["sql"].as_str().unwrap().starts_with("DELETE ")
                {
                    let expected = step["selected_prestate_ordinals"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|n| usize::try_from(n.as_u64().unwrap()).unwrap())
                        .collect::<Vec<_>>();
                    assert_eq!(
                        transition.affected_target_ordinals(),
                        expected,
                        "{id}: selected occurrences"
                    );
                }
            }
            assert_eq!(
                final_states
                    .iter()
                    .find(|state| state.relation() == "target")
                    .unwrap(),
                &state("target", &case["expected_final"])
            );
            assert_domains(
                final_states
                    .iter()
                    .find(|state| state.relation() == "target")
                    .unwrap(),
                schema,
                &case["expected_final_domains"],
            );
            assert_eq!(
                final_states
                    .iter()
                    .find(|state| state.relation() == "parent")
                    .unwrap(),
                &state("parent", &case["assumed_parent"]),
                "{id}: referenced physical source may not be mutated"
            );
        }

        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                read_native(&db),
                state("target", &step["before"]),
                "{id}: independent complete prestate #{index}"
            );
            assert_native_truth(&db, step);
            let sql = step["sql"].as_str().unwrap();
            if case["failed_step"].as_u64() == Some(index as u64) {
                assert!(
                    db.execute_batch(sql).is_err(),
                    "{id}: native constraint rejects {sql}"
                );
                assert_eq!(
                    read_native(&db),
                    state("target", &case["expected_native_target_after_failure"]),
                    "{id}: failed statement preserves whole target bag"
                );
                break;
            }
            db.execute_batch(sql).unwrap();
            assert_eq!(
                read_native(&db),
                state("target", &step["after"]),
                "{id}: independent DuckDB complete poststate #{index}"
            );
        }
    }
}
