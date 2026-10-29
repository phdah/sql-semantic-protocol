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

/// Parse the frozen independent source and target bags without consulting production
/// analysis to construct expected rows, bounds, or selected row identities.
fn frozen_rows(json: &serde_json::Value) -> Vec<(Option<i64>, i64)> {
    json.as_array()
        .expect("frozen row bag")
        .iter()
        .map(|row| {
            (
                row["id"].as_i64(),
                row["score"].as_i64().expect("frozen integer score"),
            )
        })
        .collect()
}

#[test]
fn every_frozen_exact_and_negative_cell_preserves_complete_bags_and_domains() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-150-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"], 3);
    assert_eq!(freeze["owner"], "TASK-150");
    assert_eq!(freeze["cases"].as_array().unwrap().len(), 10);
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let target_before = frozen_rows(&case["target_before"]);
        let source = case["source"]
            .as_array()
            .map(|_| frozen_rows(&case["source"]));
        let mut states = vec![state("target", &target_before)];
        if let Some(source) = &source {
            states.push(state("source", source));
        }
        let bundle = analyze(sql);
        let plan = physical_ordered_write_state_plan(&bundle, &states);
        if case["proof"] == "residual" {
            assert!(
                matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
                "{id}: {plan:?}"
            );
            continue;
        }
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = plan
        else {
            panic!("{id}: frozen exact case must be physically feasible: {plan:?}");
        };
        assert_eq!(transitions.len(), 1, "{id}");
        let transition = &transitions[0];
        let expected = frozen_rows(&case["target_after"]);
        let mut expected_sorted = expected.clone();
        expected_sorted.sort();
        assert_eq!(
            rows(transition.before()),
            rows(&state("target", &target_before)),
            "{id}"
        );
        assert_eq!(rows(transition.after()), expected_sorted, "{id}");
        assert_eq!(
            transition.affected().updated(),
            case["affected"]["updated"].as_u64().unwrap(),
            "{id}"
        );
        assert_eq!(
            transition.affected().deleted(),
            case["affected"]["deleted"].as_u64().unwrap(),
            "{id}"
        );
        if id == "81-C150-N-cte-boundary" {
            let stages = transition.intermediate_states();
            assert_eq!(stages.len(), 1, "{id}");
            assert_eq!(stages[0].relation(), "picked", "{id}");
            assert_eq!(rows(&stages[0]), vec![(Some(1), 40)], "{id}");
            let wire: serde_json::Value =
                serde_json::from_str(&sql_semantic_protocol::to_bundle_json(&bundle)).unwrap();
            let local = &wire["inputs"][0]["statements"][0]["write"]["joined_cte_source"];
            assert_eq!(local["name"], "picked");
            assert_eq!(
                local["producer"]["dependencies"],
                serde_json::json!(["source"])
            );
        }
        let selected: Vec<usize> = case["target_true_ordinals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(transition.affected_target_ordinals(), selected, "{id}");
        let not_true: Vec<usize> = (0..target_before.len())
            .filter(|index| !selected.contains(index))
            .collect();
        let frozen_not_true: Vec<usize> = case["target_not_true_ordinals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(not_true, frozen_not_true, "{id}");
        assert_eq!(
            rows(
                final_states
                    .iter()
                    .find(|s| s.relation() == "target")
                    .unwrap()
            ),
            expected_sorted,
            "{id}"
        );
        let source = source.expect("exact joined write has a complete source");
        assert_eq!(
            rows(
                final_states
                    .iter()
                    .find(|s| s.relation() == "source")
                    .unwrap()
            ),
            rows(&state("source", &source)),
            "{id}"
        );
        assert_eq!(
            rows(transition.after()),
            native(&target_before, &source, sql),
            "{id}"
        );

        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == "target")
            .unwrap();
        for domain in transition.after().exact_domains(schema).unwrap() {
            let mut observed = BTreeMap::<String, u64>::new();
            for (literal, count) in domain.values() {
                let value = match literal {
                    LiteralValue::Null => "NULL".to_string(),
                    LiteralValue::Number(value) => value.clone(),
                    other => panic!("{id}: expected signed integer domain, got {other:?}"),
                };
                observed.insert(value, *count);
            }
            let contract = &case["domain_contract"][domain.column()];
            assert!(
                contract.is_object(),
                "{id}: missing frozen domain {}",
                domain.column()
            );
            let expected: BTreeMap<String, u64> = contract["exact_frequencies"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(value, count)| (value.clone(), count.as_u64().unwrap()))
                .collect();
            assert_eq!(
                observed,
                expected,
                "{id}: typed frequency domain {}",
                domain.column()
            );
            let integers: Vec<i64> = observed
                .keys()
                .filter(|x| *x != "NULL")
                .map(|x| x.parse::<i64>().unwrap())
                .collect();
            assert_eq!(
                integers.iter().min().copied(),
                contract["lower"].as_i64(),
                "{id}"
            );
            assert_eq!(
                integers.iter().max().copied(),
                contract["upper"].as_i64(),
                "{id}"
            );
            assert_eq!(
                observed.contains_key("NULL"),
                contract["nullable"].as_bool().unwrap(),
                "{id}"
            );
            assert_eq!(contract["lower_inclusive"], true, "{id}");
            assert_eq!(contract["upper_inclusive"], true, "{id}");
        }
    }
}

#[test]
fn joined_write_wire_schema_and_json_preserve_canonical_source_identity() {
    let statement = "UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id RETURNING id,score";
    let bundle = analyze(statement);
    let emitted = sql_semantic_protocol::to_bundle_json(&bundle);
    assert_eq!(emitted, sql_semantic_protocol::to_bundle_json(&bundle));
    let json: serde_json::Value = serde_json::from_str(&emitted).unwrap();
    let query = &json["inputs"][0]["statements"][0];
    let write = &query["write"];
    assert_eq!(write["kind"], "update");
    assert_eq!(write["target"], "target");
    assert_eq!(
        write["joined_source"],
        serde_json::json!({"kind": "relation", "name": "source", "alias": "s"})
    );
    assert_eq!(
        write["returning_columns"],
        serde_json::json!(["id", "score"])
    );
    assert_eq!(query["sources"][0], write["joined_source"]);
    assert_eq!(query["dependencies"], serde_json::json!(["source"]));

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/protocol.schema.json")).unwrap();
    assert_eq!(
        schema["$defs"]["writeOperation"]["additionalProperties"],
        false
    );
    assert!(
        schema["$defs"]["writeOperation"]["properties"]["joined_source"]["$ref"]
            .as_str()
            .unwrap()
            .ends_with("/sourceRelation")
    );
    // The optional field must not be inserted into unaffected single-target writes.
    let old: serde_json::Value = serde_json::from_str(&sql_semantic_protocol::to_bundle_json(
        &analyze("UPDATE target SET score = 4 WHERE id = 1"),
    ))
    .unwrap();
    assert!(old["inputs"][0]["statements"][0]["write"]
        .get("joined_source")
        .is_none());
}

#[test]
fn joined_mutation_reads_only_the_preceding_ordered_source_state() {
    let sql = "INSERT INTO source(id,score) VALUES (1,40);
               UPDATE target AS t SET score = s.score FROM source AS s WHERE t.id = s.id";
    let bundle = analyze(sql);
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[
            state("target", &[(Some(1), 10), (Some(2), 20)]),
            state("source", &[]),
        ],
    );
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("sequential source write must be proved: {plan:?}");
    };
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0].affected().inserted(), 1);
    assert_eq!(transitions[1].affected().updated(), 1);
    assert_eq!(
        rows(
            final_states
                .iter()
                .find(|s| s.relation() == "target")
                .unwrap()
        ),
        vec![(Some(1), 40), (Some(2), 20)]
    );
    assert_eq!(
        rows(
            final_states
                .iter()
                .find(|s| s.relation() == "source")
                .unwrap()
        ),
        vec![(Some(1), 40)]
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE target(id INTEGER,score INTEGER); CREATE TABLE source(id INTEGER,score INTEGER);
        INSERT INTO target VALUES (1,10),(2,20);").unwrap();
    db.execute_batch(sql).unwrap();
    let mut stmt = db
        .prepare("SELECT id,score FROM target ORDER BY id")
        .unwrap();
    let native = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(native, vec![(1, 40), (2, 20)]);
}

#[test]
fn enforced_target_primary_key_collision_from_joined_update_is_impossible() {
    use sql_semantic_protocol::{
        ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
        RelationConstraint, RelationConstraintSet,
    };
    let sql = "UPDATE target AS t SET id = s.id FROM source AS s WHERE t.score = s.score";
    let mut bundle = analyze(sql);
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task150").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".to_string()], evidence).unwrap()],
    )
    .unwrap()]);
    let plan = physical_ordered_write_state_plan(
        &bundle,
        &[
            state("target", &[(Some(1), 10), (Some(2), 20)]),
            state("source", &[(Some(2), 10)]),
        ],
    );
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "{plan:?}"
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);
         CREATE TABLE source(id INTEGER,score INTEGER);
         INSERT INTO target VALUES (1,10),(2,20);
         INSERT INTO source VALUES (2,10);",
    )
    .unwrap();
    assert!(
        db.execute_batch(sql).is_err(),
        "native engine must reject enforced key collision"
    );
}


#[test]
fn complex_cte_mutation_is_explicitly_unsupported_not_a_silent_noop() {
    use sql_semantic_protocol::{analyze_inputs, ProtocolStatement};

    let dialect = dialect_from_name("duckdb").unwrap();
    for statement in [
        "WITH a AS (SELECT id,score FROM source),
              b AS (SELECT id,score FROM source)
         UPDATE target AS t SET score = a.score FROM a WHERE t.id = a.id",
        "WITH RECURSIVE a AS (SELECT id,score FROM source)
         UPDATE target AS t SET score = a.score FROM a WHERE t.id = a.id",
    ] {
        let analyzed = analyze_inputs(&[SqlInput::inline(statement)], "duckdb", dialect.as_ref())
            .expect("duckdb parser supports wrapped DML syntax");
        assert!(matches!(analyzed.inputs()[0].statements()[0], ProtocolStatement::Unsupported(_)),
            "unproved CTE must never be emitted as ordinary SELECT");
        let plan = physical_ordered_write_state_plan(
            &analyzed,
            &[state("target", &[(Some(1), 10)]), state("source", &[(Some(1), 40)])],
        );
        assert!(matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
            "unsupported write cannot be silently skipped: {plan:?}");
    }
}
