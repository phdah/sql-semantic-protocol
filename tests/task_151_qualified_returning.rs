//! TASK-151 bounded seam: qualified target RETURNING preserves exact physical bags.
//! Other task variants remain pending in docs/task-151-acceptance-freeze.json.

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const BEFORE: [(Option<i64>, i64); 4] = [(Some(1), 10), (Some(2), 20), (None, 30), (Some(2), 20)];

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

fn source_state(rows: &[(Option<i64>, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new("source", state(rows).rows().to_vec()).unwrap()
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
    let input = SqlInput::inline(sql);
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = ConfiguredSqlInput::new("task151", &input, "duckdb", dialect.as_ref());
    analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap()
}

fn native() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target (id INTEGER, score INTEGER);
         CREATE TABLE source (id INTEGER, score INTEGER);
         INSERT INTO target VALUES (1, 10), (2, 20), (NULL, 30), (2, 20);
         INSERT INTO source VALUES (2, 7), (2, 8), (NULL, 9);",
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
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
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
        rows(&state(&[
            (Some(1), 40),
            (Some(2), 40),
            (None, 30),
            (Some(2), 40)
        ])),
    );
    let returned = transition.returning().expect("RETURNING is present");
    let returned_ids = returned
        .rows()
        .iter()
        .map(|row| match row.values().get("id").unwrap() {
            LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
            other => panic!("unexpected returned id: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(returned_ids, vec![1, 2, 2]);
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "target")
        .unwrap();
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
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("qualified DELETE RETURNING should be exact: {plan:?}");
    };
    let [transition] = transitions.as_slice() else {
        panic!("expected one write transition");
    };
    assert_eq!(transition.affected().deleted(), 2);
    assert_eq!(transition.affected_target_ordinals(), &[1, 3]);
    assert_eq!(final_states, vec![transition.after().clone()]);
    assert_eq!(
        rows(transition.after()),
        rows(&state(&[(Some(1), 10), (None, 30)]))
    );
    let returned = transition.returning().expect("RETURNING is present");
    let returned_rows = returned
        .rows()
        .iter()
        .map(|row| {
            let id = match row.values().get("id").unwrap() {
                LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
                other => panic!("unexpected returned id: {other:?}"),
            };
            let score = match row.values().get("score").unwrap() {
                LiteralValue::Number(value) => value.parse::<i64>().unwrap(),
                other => panic!("unexpected returned score: {other:?}"),
            };
            (id, score)
        })
        .collect::<Vec<_>>();
    assert_eq!(returned_rows, vec![(2, 20), (2, 20)]);
    let domains = transition
        .returning_exact_domains(
            bundle
                .source_schemas()
                .iter()
                .find(|schema| schema.relation() == "target")
                .unwrap(),
        )
        .unwrap();
    assert_eq!(domains.len(), 2);
    assert_eq!(
        domains[0].values(),
        &[(LiteralValue::Number("2".to_string()), 2)]
    );
    assert_eq!(
        domains[1].values(),
        &[(LiteralValue::Number("20".to_string()), 2)]
    );
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
        "UPDATE target AS t SET score = 40 RETURNING target.id",
        "DELETE FROM target WHERE id = 2 RETURNING *",
    ] {
        let plan = physical_ordered_write_state_plan(&bundle(sql), &[state(&BEFORE)]);
        assert!(
            matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
            "{sql}: {plan:?}"
        );
    }
}

#[test]
fn alias_and_unqualified_returning_collision_is_rejected() {
    let sql = "UPDATE target AS t SET score = 40 RETURNING t.id, id";
    let plan = physical_ordered_write_state_plan(&bundle(sql), &[state(&BEFORE)]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

#[test]
fn correlated_exists_update_uses_complete_read_only_bag_and_returns_postimages() {
    let sql = "UPDATE target AS t SET score = 99 WHERE EXISTS (SELECT 1 FROM source AS s WHERE s.id = t.id) RETURNING t.id";
    let source = source_state(&[(Some(2), 7), (Some(2), 8), (None, 9)]);
    let bundle = bundle(sql);
    let plan = physical_ordered_write_state_plan(&bundle, &[state(&BEFORE), source.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("complete correlated EXISTS must be exact: {plan:?}");
    };
    let [transition] = transitions.as_slice() else {
        panic!("one transition");
    };
    assert_eq!(transition.affected().updated(), 2);
    assert_eq!(transition.affected_target_ordinals(), &[1, 3]);
    assert_eq!(rows(transition.before()), rows(&state(&BEFORE)));
    assert_eq!(
        rows(transition.after()),
        rows(&state(&[
            (Some(1), 10),
            (Some(2), 99),
            (None, 30),
            (Some(2), 99)
        ]))
    );
    let returning = transition.returning().unwrap();
    assert_eq!(returning.rows().len(), 2);
    let domains = transition
        .returning_exact_domains(
            bundle
                .source_schemas()
                .iter()
                .find(|schema| schema.relation() == "target")
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        domains[0].values(),
        &[(LiteralValue::Number("2".into()), 2)]
    );
    assert_eq!(final_states.len(), 2);
    assert!(final_states.contains(&source));
    let native = native();
    let mut stmt = native.prepare(sql).unwrap();
    let actual = stmt
        .query_map([], |r| r.get::<_, i64>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    drop(stmt);
    assert_eq!(actual, vec![2, 2]);
    assert_eq!(rows(transition.after()), native_target_rows(&native));
}

#[test]
fn correlated_not_exists_excludes_duplicates_and_sql_unknown_without_inventing_rows() {
    let sql =
        "DELETE FROM target AS t WHERE NOT EXISTS (SELECT 1 FROM source AS s WHERE s.id = t.id)";
    let source = source_state(&[(Some(2), 7), (Some(2), 8), (None, 9)]);
    let plan = physical_ordered_write_state_plan(&bundle(sql), &[state(&BEFORE), source]);
    let PhysicalWriteStatePlan::Feasible { transitions, .. } = plan else {
        panic!("complete NOT EXISTS is independently provable: {plan:?}");
    };
    assert_eq!(transitions[0].affected_target_ordinals(), &[0, 2]);
    assert_eq!(transitions[0].affected().deleted(), 2);
    assert_eq!(
        rows(transitions[0].after()),
        rows(&state(&[(Some(2), 20), (Some(2), 20)]))
    );
    let native = native();
    native.execute_batch(sql).unwrap();
    assert_eq!(rows(transitions[0].after()), native_target_rows(&native));
}

#[test]
fn correlated_source_is_mandatory_not_an_implicitly_empty_relation() {
    let sql = "DELETE FROM target AS t WHERE EXISTS (SELECT 1 FROM source AS s WHERE s.id = t.id)";
    let plan = physical_ordered_write_state_plan(&bundle(sql), &[state(&BEFORE)]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
        "{plan:?}"
    );
}

fn frozen_state(relation: &str, input: &serde_json::Value) -> PhysicalWriteState {
    let rows = input.as_array().expect("frozen complete row bag");
    PhysicalWriteState::new(
        relation,
        rows.iter()
            .map(|row| {
                let mut values = BTreeMap::new();
                for (column, value) in row.as_object().unwrap() {
                    let typed = if value.is_null() {
                        LiteralValue::Null
                    } else {
                        LiteralValue::Number(value.as_i64().expect("frozen integer").to_string())
                    };
                    values.insert(column.clone(), typed);
                }
                PhysicalWriteRow::new(values).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn frozen_exact_cases_have_complete_prestate_poststate_and_typed_domains() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-151-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["owner"], "TASK-151");
    let mut checked = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["proof"] != "exact" {
            continue;
        }
        checked += 1;
        let sql = case["sql"].as_str().unwrap();
        let bundle = bundle(sql);
        let before = frozen_state("target", &case["target_before"]);
        let mut states = vec![before.clone()];
        let source = if case["source"].is_array() {
            let source = frozen_state("source", &case["source"]);
            states.push(source.clone());
            Some(source)
        } else {
            None
        };
        let plan = physical_ordered_write_state_plan(&bundle, &states);
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = plan
        else {
            panic!("{}: required exact witness: {plan:?}", case["id"]);
        };
        assert_eq!(transitions.len(), 1, "{}", case["id"]);
        let transition = &transitions[0];
        assert_eq!(transition.before(), &before, "{}", case["id"]);
        let expected = frozen_state("target", &case["target_after"]);
        assert_eq!(transition.after(), &expected, "{}", case["id"]);
        assert!(final_states.contains(&expected), "{}", case["id"]);
        if let Some(source) = source {
            assert!(final_states.contains(&source), "{}", case["id"]);
        }
        assert_eq!(
            transition.affected().updated(),
            case["affected"]["updated"].as_u64().unwrap()
        );
        assert_eq!(
            transition.affected().deleted(),
            case["affected"]["deleted"].as_u64().unwrap()
        );
        let selected = case["target_true_ordinals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        assert_eq!(
            transition.affected_target_ordinals(),
            selected.as_slice(),
            "{}",
            case["id"]
        );
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == "target")
            .unwrap();
        let domains = transition.after().exact_domains(schema).unwrap();
        for column in domains {
            let expected = &case["domain_contract"][column.column()];
            let frequencies = expected["frequencies"].as_object().unwrap();
            let mut actual = BTreeMap::new();
            for (value, count) in column.values() {
                let key = match value {
                    LiteralValue::Null => "NULL".to_string(),
                    LiteralValue::Number(value) => value.clone(),
                    other => panic!("unproved frozen type: {other:?}"),
                };
                actual.insert(key, *count);
            }
            assert_eq!(actual.len(), frequencies.len(), "{}", case["id"]);
            for (value, count) in actual {
                assert_eq!(Some(count), frequencies[&value].as_u64(), "{}", case["id"]);
            }
            assert_eq!(expected["lower_inclusive"], true);
            assert_eq!(expected["upper_inclusive"], true);
        }
        if !case["returning"].as_array().unwrap().is_empty() {
            let returning = transition.returning().expect("required RETURNING bag");
            let expected_returning = frozen_state("target", &case["returning"]);
            assert_eq!(
                returning.rows(),
                expected_returning.rows(),
                "{}",
                case["id"]
            );
            assert_eq!(
                transition.returning_exact_domains(schema).unwrap().len(),
                case["returning"][0].as_object().unwrap().len()
            );
        } else {
            assert!(transition.returning().is_none(), "{}", case["id"]);
        }
    }
    assert_eq!(checked, 4);
}

#[test]
fn correlated_update_enforced_key_collision_is_impossible_and_native_rejects() {
    let sql =
        "UPDATE target AS t SET id = 2 WHERE EXISTS (SELECT 1 FROM source AS s WHERE s.id = t.id)";
    let mut analyzed = bundle(sql);
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task151-pk").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    analyzed.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".into()], evidence).unwrap()],
    )
    .unwrap()]);
    let before = state(&[(Some(1), 10), (Some(2), 20)]);
    let source = source_state(&[(Some(1), 7)]);
    let plan = physical_ordered_write_state_plan(&analyzed, &[before.clone(), source]);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "{plan:?}"
    );

    let native = Connection::open_in_memory().unwrap();
    native
        .execute_batch(
            "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
         CREATE TABLE source(id INTEGER, score INTEGER);
         INSERT INTO target VALUES (1,10),(2,20);
         INSERT INTO source VALUES (1,7);",
        )
        .unwrap();
    assert!(native.execute_batch(sql).is_err());
    assert_eq!(native_target_rows(&native), rows(&before));
}
