//! TASK-155: repeated MERGE source matches and narrow ON CONFLICT equivalence.
//! The fixture contract is independent of the interpreter's output and records
//! both physical inputs, poststate domains and fail-closed classifications.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput,
};

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-155-acceptance-freeze.json")).unwrap()
}

fn row(value: &Value) -> PhysicalWriteRow {
    let values = value
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                value
                    .as_i64()
                    .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
            )
        })
        .collect();
    PhysicalWriteRow::new(values).unwrap()
}

fn state(relation: &str, rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(relation, rows.as_array().unwrap().iter().map(row).collect()).unwrap()
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&["target", "source"].map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "score"]
                .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap())
                .to_vec(),
        )
        .unwrap()
    }))
    .unwrap()
}

fn analyzed(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = ConfiguredSqlInput::new("task155", &sql, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog()).unwrap();

    let mut constraints = Vec::new();
    for relation in ["target", "source"] {
        if case[format!("{relation}_pk").as_str()] != true {
            continue;
        }
        let evidence = ConstraintEvidence::new(
            ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-155").unwrap(),
            ConstraintEnforcement::Enforced,
        );
        constraints.push(
            RelationConstraintSet::new(
                relation,
                vec![
                    RelationConstraint::primary_key(vec!["id".to_string()], vec![evidence])
                        .unwrap(),
                ],
            )
            .unwrap(),
        );
    }
    bundle.enrich_relation_constraints(&constraints);
    bundle
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    let mut snapshots = vec![state("target", &case["target_before"])];
    if case["sql"].as_str().unwrap().starts_with("MERGE") {
        snapshots.push(state("source", &case["source"]));
    }
    snapshots
}

fn sorted(state: &PhysicalWriteState) -> Vec<String> {
    let mut rows = state
        .rows()
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn native(case: &Value) -> duckdb::Result<Connection> {
    let db = Connection::open_in_memory()?;
    let target_pk = if case["target_pk"] == true {
        " PRIMARY KEY"
    } else {
        ""
    };
    let source_pk = if case["source_pk"] == true {
        " PRIMARY KEY"
    } else {
        ""
    };
    db.execute_batch(&format!(
        "CREATE TABLE target(id INTEGER{target_pk}, score INTEGER);
         CREATE TABLE source(id INTEGER{source_pk}, score INTEGER);"
    ))?;
    for (relation, rows) in [
        ("target", &case["target_before"]),
        ("source", &case["source"]),
    ] {
        for value in rows.as_array().unwrap() {
            db.execute(
                &format!("INSERT INTO {relation}(id,score) VALUES (?1,?2)"),
                params![value["id"].as_i64(), value["score"].as_i64()],
            )?;
        }
    }
    Ok(db)
}

fn native_state(db: &Connection, relation: &str) -> PhysicalWriteState {
    let mut query = db
        .prepare(&format!("SELECT id, score FROM {relation}"))
        .unwrap();
    let rows = query
        .query_map([], |r| {
            Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .map(|(id, score)| {
            PhysicalWriteRow::new(BTreeMap::from([
                (
                    "id".to_string(),
                    id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                ),
                (
                    "score".to_string(),
                    score.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                ),
            ]))
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new(relation, rows).unwrap()
}

fn domains(state: &PhysicalWriteState, expected: &Value, schema: &RelationSchema) {
    let actual = state.exact_domains(schema).unwrap();
    for column in actual {
        let want = &expected[column.column()];
        assert_eq!(want["type"], "INTEGER");
        let frequencies = column
            .values()
            .iter()
            .map(|(value, count)| {
                let key = match value {
                    LiteralValue::Null => "NULL".to_string(),
                    LiteralValue::Number(n) => n.clone(),
                    other => panic!("unexpected typed INTEGER {other:?}"),
                };
                (key, *count)
            })
            .collect::<BTreeMap<_, _>>();
        let expected_frequencies = want["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, count)| (key.clone(), count.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(frequencies, expected_frequencies);
        let numbers = frequencies
            .keys()
            .filter(|key| key.as_str() != "NULL")
            .map(|key| key.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(numbers.iter().min().copied(), want["lower"].as_i64());
        assert_eq!(numbers.iter().max().copied(), want["upper"].as_i64());
        assert_eq!(frequencies.contains_key("NULL"), want["nullable"]);
        assert_eq!(!numbers.is_empty(), want["lower_inclusive"]);
        assert_eq!(!numbers.is_empty(), want["upper_inclusive"]);
    }
}

fn on_ordinals(db: &Connection, relation: &str, not_true: bool) -> Vec<usize> {
    let opposite = if relation == "target" {
        "source"
    } else {
        "target"
    };
    let predicate = if not_true { "NOT EXISTS" } else { "EXISTS" };
    let query = format!(
        "SELECT ordinal FROM (
           SELECT row_number() OVER (ORDER BY rowid)-1 AS ordinal, id FROM {relation}
         ) AS t WHERE {predicate} (SELECT 1 FROM {opposite} s WHERE t.id=s.id)
         ORDER BY ordinal"
    );
    let mut stmt = db.prepare(&query).unwrap();
    stmt.query_map([], |r| r.get::<_, i64>(0))
        .unwrap()
        .map(Result::unwrap)
        .map(|n| usize::try_from(n).unwrap())
        .collect()
}

fn ordinals(value: &Value) -> Vec<usize> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| usize::try_from(v.as_u64().unwrap()).unwrap())
        .collect()
}

#[test]
fn frozen_repeated_match_and_conflict_cases_prove_only_attested_exact_poststates() {
    let freeze = frozen();
    assert_eq!(freeze["version"], 4);
    assert_eq!(freeze["cases"].as_array().unwrap().len(), 12);
    for case in freeze["cases"].as_array().unwrap().iter().take(7) {
        let id = case["id"].as_str().unwrap();
        let bundle = analyzed(case);
        let initial = snapshots(case);
        let before = &initial[0];
        let source = state("source", &case["source"]);
        let target_schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == "target")
            .unwrap();
        let source_schema = RelationSchema::new(
            "source",
            ["id", "score"]
                .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", "duckdb").unwrap())
                .to_vec(),
        )
        .unwrap();
        domains(before, &case["before_domains"], target_schema);
        domains(&source, &case["source_domains"], &source_schema);
        let plan = physical_ordered_write_state_plan(&bundle, &initial);

        match case["classification"].as_str().unwrap() {
            "Exact" => {
                let PhysicalWriteStatePlan::Feasible {
                    transitions,
                    final_states,
                } = plan
                else {
                    panic!("{id}: expected Exact, got {plan:?}");
                };
                assert_eq!(transitions.len(), 1, "{id}: one atomic statement");
                let transition = &transitions[0];
                let expected = state("target", &case["target_after"]);
                assert_eq!(sorted(transition.before()), sorted(before));
                assert_eq!(sorted(transition.after()), sorted(&expected));
                assert_eq!(
                    transition.affected_target_ordinals(),
                    ordinals(&case["on_true_target_ordinals"]),
                    "{id}: target physical occurrence identity"
                );
                assert_eq!(
                    transition.affected().inserted(),
                    case["affected"]["inserted"].as_u64().unwrap()
                );
                assert_eq!(
                    transition.affected().updated(),
                    case["affected"]["updated"].as_u64().unwrap()
                );
                assert_eq!(
                    transition.affected().deleted(),
                    case["affected"]["deleted"].as_u64().unwrap()
                );
                domains(transition.after(), &case["after_domains"], target_schema);
                if case["sql"].as_str().unwrap().starts_with("MERGE") {
                    assert_eq!(
                        sorted(
                            final_states
                                .iter()
                                .find(|s| s.relation() == "source")
                                .unwrap()
                        ),
                        sorted(&source),
                        "{id}: MERGE source bag is never rewritten"
                    );
                }

                let db = native(case).unwrap();
                assert_eq!(sorted(&native_state(&db, "target")), sorted(before));
                assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                if case["sql"].as_str().unwrap().starts_with("MERGE") {
                    assert_eq!(
                        on_ordinals(&db, "target", false),
                        ordinals(&case["on_true_target_ordinals"]),
                        "{id}: independent ON TRUE"
                    );
                    assert_eq!(
                        on_ordinals(&db, "target", true),
                        ordinals(&case["on_not_true_target_ordinals"]),
                        "{id}: independent ON NOT TRUE"
                    );
                    assert_eq!(
                        on_ordinals(&db, "source", false),
                        ordinals(&case["on_true_source_ordinals"]),
                        "{id}: independent source ON TRUE"
                    );
                    assert_eq!(
                        on_ordinals(&db, "source", true),
                        ordinals(&case["on_not_true_source_ordinals"]),
                        "{id}: independent source ON NOT TRUE"
                    );
                }
                db.execute_batch(case["sql"].as_str().unwrap()).unwrap();
                assert_eq!(sorted(&native_state(&db, "target")), sorted(&expected));
                assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
            }
            "Impossible" => {
                assert!(
                    matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
                    "{id}: expected Impossible, got {plan:?}"
                );
                if case["source_pk"] == true {
                    // A duplicate authoritative source key cannot form an initial SQL table.
                    assert!(native(case).is_err(), "{id}: invalid closed source bag");
                } else {
                    let db = native(case).unwrap();
                    assert_eq!(
                        on_ordinals(&db, "target", false),
                        ordinals(&case["on_true_target_ordinals"])
                    );
                    assert_eq!(
                        on_ordinals(&db, "target", true),
                        ordinals(&case["on_not_true_target_ordinals"])
                    );
                    assert_eq!(
                        on_ordinals(&db, "source", false),
                        ordinals(&case["on_true_source_ordinals"])
                    );
                    assert_eq!(
                        on_ordinals(&db, "source", true),
                        ordinals(&case["on_not_true_source_ordinals"])
                    );
                    assert!(db.execute_batch(case["sql"].as_str().unwrap()).is_err());
                    assert_eq!(sorted(&native_state(&db, "target")), sorted(before));
                    assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                }
            }
            "Residual" => {
                let PhysicalWriteStatePlan::Residual { gap, .. } = plan else {
                    panic!("{id}: expected Residual, got {plan:?}");
                };
                assert_eq!(format!("{gap:?}"), case["gap"].as_str().unwrap(), "{id}");
                if id == "82-C155-N-ambiguous-winner" {
                    let db = native(case).unwrap();
                    assert_eq!(sorted(&native_state(&db, "target")), sorted(before));
                    assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                    assert_eq!(
                        on_ordinals(&db, "target", false),
                        ordinals(&case["on_true_target_ordinals"])
                    );
                    assert_eq!(
                        on_ordinals(&db, "target", true),
                        ordinals(&case["on_not_true_target_ordinals"])
                    );
                    assert_eq!(
                        on_ordinals(&db, "source", false),
                        ordinals(&case["on_true_source_ordinals"])
                    );
                    assert_eq!(
                        on_ordinals(&db, "source", true),
                        ordinals(&case["on_not_true_source_ordinals"])
                    );
                    let result = db.execute_batch(case["sql"].as_str().unwrap());
                    let after = native_state(&db, "target");
                    if result.is_err() {
                        assert_eq!(sorted(&after), sorted(before), "{id}: atomic failure");
                    } else {
                        // DuckDB may select either original source occurrence. Assert the
                        // entire observed bag without granting a portable winner law.
                        let candidates = [
                            serde_json::json!([{"id": 1, "score": 20}]),
                            serde_json::json!([{"id": 1, "score": 30}]),
                        ];
                        assert!(
                            candidates
                                .iter()
                                .any(|candidate| sorted(&after) == sorted(&state("target", candidate))),
                            "{id}: native success must preserve exactly one original target row"
                        );
                    }
                    assert_eq!(
                        sorted(&native_state(&db, "source")),
                        sorted(&source),
                        "{id}: ambiguous write cannot modify source"
                    );
                } else if id == "82-C155-N-upsert-update" {
                    let db = native(case).unwrap();
                    assert_eq!(sorted(&native_state(&db, "target")), sorted(before));
                    assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                    db.execute_batch(case["sql"].as_str().unwrap()).unwrap();
                    // This is a DuckDB-only observation, not canonical evidence
                    // for treating ON CONFLICT DO UPDATE as a portable MERGE.
                    let native_after =
                        state("target", &serde_json::json!([{"id": 1, "score": 99}]));
                    assert_eq!(sorted(&native_state(&db, "target")), sorted(&native_after));
                    assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                }
            }
            other => panic!("{id}: unexpected proof class {other}"),
        }
    }
}

#[test]
fn ordered_conflict_then_matched_merge_reuses_one_complete_target_and_source_bag() {
    let freeze = frozen();
    let replay = &freeze["ordered_replay"];
    assert_eq!(replay["id"], "82-C155-P-ordered-conflict-merge");
    let steps = replay["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2);
    let sql_inputs = steps
        .iter()
        .map(|step| SqlInput::inline(step["sql"].as_str().unwrap()))
        .collect::<Vec<_>>();
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = sql_inputs
        .iter()
        .enumerate()
        .map(|(i, input)| {
            ConfiguredSqlInput::new(
                if i == 0 {
                    "task155-step0"
                } else {
                    "task155-step1"
                },
                input,
                "duckdb",
                dialect.as_ref(),
            )
        })
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap();
    let enforced = ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task155-replay")
            .unwrap(),
        ConstraintEnforcement::Enforced,
    );
    bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
        "target",
        vec![RelationConstraint::primary_key(vec!["id".into()], vec![enforced]).unwrap()],
    )
    .unwrap()]);
    let target = state("target", &replay["target_initial"]);
    let source = state("source", &replay["source"]);
    let plan = physical_ordered_write_state_plan(&bundle, &[target.clone(), source.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("ordered conflict and MERGE replay must be exact: {plan:?}");
    };
    assert_eq!(transitions.len(), 2);
    for (transition, step) in transitions.iter().zip(steps) {
        assert_eq!(
            sorted(transition.before()),
            sorted(&state("target", &step["before"]))
        );
        assert_eq!(
            sorted(transition.after()),
            sorted(&state("target", &step["after"]))
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
        assert_eq!(
            transition.affected_target_ordinals(),
            ordinals(&step["affected_target_ordinals"])
        );
        assert!(
            transition.intermediate_states().is_empty(),
            "an intermediate physical target cannot become an independent source"
        );
    }
    assert_eq!(transitions[0].after(), transitions[1].before());
    assert_eq!(
        final_states.iter().find(|s| s.relation() == "source"),
        Some(&source)
    );
    assert_eq!(
        sorted(
            final_states
                .iter()
                .find(|s| s.relation() == "target")
                .unwrap()
        ),
        sorted(&state("target", &replay["terminal"]))
    );

    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
         CREATE TABLE source(id INTEGER, score INTEGER);
         INSERT INTO target VALUES (1,10),(2,20);
         INSERT INTO source VALUES (1,30);",
    )
    .unwrap();
    assert_eq!(sorted(&native_state(&db, "target")), sorted(&target));
    assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
    for step in steps {
        db.execute_batch(step["sql"].as_str().unwrap()).unwrap();
        assert_eq!(
            sorted(&native_state(&db, "target")),
            sorted(&state("target", &step["after"])),
            "native ordered state must match the frozen complete bag"
        );
        assert_eq!(
            sorted(&native_state(&db, "source")),
            sorted(&source),
            "the independent source population is immutable"
        );
    }
}
