//! TASK-154: unmatched MERGE branch proofs against complete physical bags.
//! Expected states and nullable inclusive domains come from the frozen contract.

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

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-154-acceptance-freeze.json")).unwrap()
}

fn row(v: &Value) -> PhysicalWriteRow {
    PhysicalWriteRow::new(
        v.as_object()
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
            .collect(),
    )
    .unwrap()
}

fn state(relation: &str, v: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(relation, v.as_array().unwrap().iter().map(row).collect()).unwrap()
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

fn analyze(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = ConfiguredSqlInput::new("task154", &sql, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog()).unwrap();
    if case["pk"] == true {
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::primary_key(
                vec!["id".to_string()],
                vec![ConstraintEvidence::new(
                    ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-154")
                        .unwrap(),
                    ConstraintEnforcement::Enforced,
                )],
            )
            .unwrap()],
        )
        .unwrap()]);
    }
    bundle
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    let mut states = vec![state("target", &case["target_before"])];
    if case["source"].is_array() {
        states.push(state("source", &case["source"]));
    }
    states
}

fn native(case: &Value) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    if case["pk"] == true {
        db.execute_batch(
            "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);
             CREATE TABLE source(id INTEGER, score INTEGER);",
        )
        .unwrap();
    } else {
        db.execute_batch(
            "CREATE TABLE target(id INTEGER, score INTEGER);
             CREATE TABLE source(id INTEGER, score INTEGER);",
        )
        .unwrap();
    }
    for row in case["target_before"].as_array().unwrap() {
        db.execute(
            "INSERT INTO target(id,score) VALUES (?1,?2)",
            params![row["id"].as_i64(), row["score"].as_i64()],
        )
        .unwrap();
    }
    if let Some(rows) = case["source"].as_array() {
        for row in rows {
            db.execute(
                "INSERT INTO source(id,score) VALUES (?1,?2)",
                params![row["id"].as_i64(), row["score"].as_i64()],
            )
            .unwrap();
        }
    }
    db
}

// SQL tables are bags, not ordered vectors. Preserve multiplicities and NULLs
// while avoiding rowid-order assumptions when INSERT and DELETE occur together.
fn sorted_rows(s: &PhysicalWriteState) -> Vec<String> {
    let mut rows = s
        .rows()
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn native_state(db: &Connection, relation: &str) -> PhysicalWriteState {
    let mut query = db
        .prepare(&format!("SELECT id,score FROM {relation}"))
        .unwrap();
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .map(|(id, score)| {
            PhysicalWriteRow::new(BTreeMap::from([
                (
                    "id".to_string(),
                    id.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                ),
                (
                    "score".to_string(),
                    score.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                ),
            ]))
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new(relation, rows).unwrap()
}

// This independent native SQL oracle checks both ON directions. UNKNOWN from
// NULL behaves as NOT TRUE; a NOT MATCHED source row is never a target row.
fn native_on_ordinals(db: &Connection, relation: &str, not_true: bool) -> Vec<usize> {
    let other = if relation == "target" {
        "source"
    } else {
        "target"
    };
    let predicate = if not_true { "NOT EXISTS" } else { "EXISTS" };
    let sql = format!(
        "SELECT ordinal FROM (
           SELECT row_number() OVER (ORDER BY rowid) - 1 AS ordinal, id FROM {relation}
         ) AS physical
         WHERE {predicate} (SELECT 1 FROM {other} AS peer WHERE physical.id = peer.id)
         ORDER BY ordinal"
    );
    let mut query = db.prepare(&sql).unwrap();
    query
        .query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .map(Result::unwrap)
        .map(|v| usize::try_from(v).unwrap())
        .collect()
}

fn ordinals(v: &Value) -> Vec<usize> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| usize::try_from(v.as_u64().unwrap()).unwrap())
        .collect()
}

fn assert_domains(state: &PhysicalWriteState, schema: &RelationSchema, expected: &Value) {
    let domains = state.exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 2);
    for column in domains {
        let frozen = &expected[column.column()];
        assert_eq!(frozen["type"], "INTEGER");
        let frequencies = column
            .values()
            .iter()
            .map(|(v, count)| {
                let value = match v {
                    LiteralValue::Number(v) => v.clone(),
                    LiteralValue::Null => "NULL".to_string(),
                    other => panic!("unexpected integer column value: {other:?}"),
                };
                (value, *count)
            })
            .collect::<BTreeMap<_, _>>();
        let expected_frequencies = frozen["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(value, count)| (value.clone(), count.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(frequencies, expected_frequencies, "{}", column.column());
        let values = frequencies
            .keys()
            .filter(|value| value.as_str() != "NULL")
            .map(|value| value.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values.iter().min().copied(), frozen["lower"].as_i64());
        assert_eq!(values.iter().max().copied(), frozen["upper"].as_i64());
        assert_eq!(frequencies.contains_key("NULL"), frozen["nullable"]);
        assert_eq!(!values.is_empty(), frozen["lower_inclusive"]);
        assert_eq!(!values.is_empty(), frozen["upper_inclusive"]);
    }
}

#[test]
fn frozen_unmatched_merge_cases_prove_both_complete_bags_and_both_on_directions() {
    let frozen = freeze();
    assert_eq!(frozen["version"], 3);
    let cases = frozen["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    let exact = cases.iter().filter(|case| case["proof"] == "Exact");
    let mut proven = 0;
    for case in exact {
        proven += 1;
        let id = case["id"].as_str().unwrap();
        let bundle = analyze(case);
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == "target")
            .unwrap();
        let source_schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == "source")
            .unwrap();
        let before = state("target", &case["target_before"]);
        let source = state("source", &case["source"]);
        let after = state("target", &case["target_after"]);
        let db = native(case);
        assert_eq!(
            sorted_rows(&native_state(&db, "target")),
            sorted_rows(&before),
            "{id}: native before"
        );
        assert_eq!(
            sorted_rows(&native_state(&db, "source")),
            sorted_rows(&source),
            "{id}: source before"
        );

        assert_eq!(
            native_on_ordinals(&db, "target", false),
            ordinals(&case["on_true_target_ordinals"]),
            "{id}: target ON TRUE"
        );
        assert_eq!(
            native_on_ordinals(&db, "target", true),
            ordinals(&case["on_not_true_target_ordinals"]),
            "{id}: target NOT TRUE"
        );
        assert_eq!(
            native_on_ordinals(&db, "source", false),
            ordinals(&case["on_true_source_ordinals"]),
            "{id}: source ON TRUE"
        );
        assert_eq!(
            native_on_ordinals(&db, "source", true),
            ordinals(&case["on_not_true_source_ordinals"]),
            "{id}: source NOT TRUE"
        );

        assert_domains(&before, schema, &case["before_domains"]);
        assert_domains(&source, source_schema, &case["source_domains"]);
        let plan = physical_ordered_write_state_plan(&bundle, &snapshots(case));
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = &plan
        else {
            panic!("{id}: expected exact complete MERGE, got {plan:?}");
        };
        assert_eq!(transitions.len(), 1, "{id}: one atomic MERGE");
        let step = &transitions[0];
        assert_eq!(step.before(), &before, "{id}: canonical prestate");
        assert_eq!(
            sorted_rows(step.after()),
            sorted_rows(&after),
            "{id}: canonical poststate"
        );
        assert_eq!(
            step.affected().inserted(),
            case["affected"]["inserted"].as_u64().unwrap(),
            "{id}: inserts"
        );
        assert_eq!(
            step.affected().updated(),
            case["affected"]["updated"].as_u64().unwrap(),
            "{id}: updates"
        );
        assert_eq!(
            step.affected().deleted(),
            case["affected"]["deleted"].as_u64().unwrap(),
            "{id}: deletes"
        );
        assert_eq!(
            step.affected_target_ordinals(),
            ordinals(&case["affected_target_ordinals"]),
            "{id}: prestate identities"
        );
        assert!(
            step.intermediate_states().is_empty(),
            "{id}: no invented intermediate source"
        );
        assert_domains(step.after(), schema, &case["after_domains"]);
        db.execute_batch(case["sql"].as_str().unwrap()).unwrap();
        let native_after = native_state(&db, "target");
        assert_eq!(
            sorted_rows(&native_after),
            sorted_rows(&after),
            "{id}: DuckDB poststate"
        );
        assert_domains(&native_after, schema, &case["after_domains"]);
        assert_eq!(
            sorted_rows(&native_state(&db, "source")),
            sorted_rows(&source),
            "{id}: unchanged physical source"
        );
        assert_eq!(
            sorted_rows(
                final_states
                    .iter()
                    .find(|s| s.relation() == "target")
                    .unwrap()
            ),
            sorted_rows(&after),
            "{id}: final target"
        );
        assert_eq!(
            sorted_rows(
                final_states
                    .iter()
                    .find(|s| s.relation() == "source")
                    .unwrap()
            ),
            sorted_rows(&source),
            "{id}: final source"
        );
    }
    assert_eq!(proven, 9);
}

#[test]
fn frozen_negative_merges_fail_closed_and_preserve_native_atomicity() {
    let frozen = freeze();
    for case in frozen["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["proof"] != "Exact")
    {
        let id = case["id"].as_str().unwrap();
        let plan = physical_ordered_write_state_plan(&analyze(case), &snapshots(case));
        match case["proof"].as_str().unwrap() {
            "Residual" => {
                let PhysicalWriteStatePlan::Residual { gap, .. } = plan else {
                    panic!("{id}: expected typed Residual: {plan:?}");
                };
                assert_eq!(format!("{gap:?}"), case["gap"].as_str().unwrap(), "{id}");
                if id == "82-C154-N-duplicate-match" {
                    let db = native(case);
                    let before = state("target", &case["target_before"]);
                    let source = state("source", &case["source"]);
                    let executed = db.execute_batch(case["sql"].as_str().unwrap());
                    let observed = native_state(&db, "target");
                    if executed.is_err() {
                        assert_eq!(
                            sorted_rows(&observed),
                            sorted_rows(&before),
                            "{id}: native failed statement is atomic"
                        );
                    } else {
                        // A native success may select either duplicate source
                        // winner. It does not establish portable exactness.
                        let winners = case["source"].as_array().unwrap();
                        let candidates = winners
                            .iter()
                            .filter(|winner| winner["id"] == 1)
                            .map(|winner| {
                                let mut candidate = case["target_before"].clone();
                                candidate[0]["score"] = winner["score"].clone();
                                candidate
                                    .as_array_mut()
                                    .unwrap()
                                    .push(serde_json::json!({"id": 3, "score": 300}));
                                state("target", &candidate)
                            })
                            .collect::<Vec<_>>();
                        assert!(
                            candidates
                                .iter()
                                .any(|candidate| sorted_rows(candidate) == sorted_rows(&observed)),
                            "{id}: native success must have a permitted but unproven source winner"
                        );
                    }
                    assert_eq!(
                        sorted_rows(&native_state(&db, "source")),
                        sorted_rows(&source),
                        "{id}: complete independent source is never mutated"
                    );
                }
            }
            "Impossible" => {
                assert!(
                    matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
                    "{id}: {plan:?}"
                );
                let db = native(case);
                let before = state("target", &case["target_before"]);
                let source = state("source", &case["source"]);
                assert!(
                    db.execute_batch(case["sql"].as_str().unwrap()).is_err(),
                    "{id}: native key collision"
                );
                assert_eq!(
                    sorted_rows(&native_state(&db, "target")),
                    sorted_rows(&before),
                    "{id}: atomic failure target"
                );
                assert_eq!(
                    sorted_rows(&native_state(&db, "source")),
                    sorted_rows(&source),
                    "{id}: untouched source"
                );
            }
            other => panic!("{id}: unexpected proof classification {other}"),
        }
    }
}

#[test]
fn ordered_unmatched_merges_replay_without_invented_intermediate_sources() {
    let frozen = freeze();
    let replay = &frozen["ordered_replay"];
    assert_eq!(replay["id"], "82-C154-P-ordered-replay");
    assert_eq!(replay["proof"], "Exact");
    let steps = replay["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);

    let inputs = steps
        .iter()
        .map(|step| SqlInput::inline(step["sql"].as_str().unwrap()))
        .collect::<Vec<_>>();
    let ids = (0..inputs.len())
        .map(|index| format!("task154-ordered-{index}"))
        .collect::<Vec<_>>();
    let parser = dialect_from_name("duckdb").unwrap();
    let configured = inputs
        .iter()
        .zip(&ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "duckdb", parser.as_ref()))
        .collect::<Vec<_>>();
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap();
    let initial = state("target", &replay["target_initial"]);
    let source = state("source", &replay["source"]);
    let plan = physical_ordered_write_state_plan(&bundle, &[initial.clone(), source.clone()]);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = &plan
    else {
        panic!("expected exact ordered MERGE replay: {plan:?}");
    };
    assert_eq!(transitions.len(), steps.len());
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "target")
        .unwrap();
    let source_schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "source")
        .unwrap();
    assert_domains(&source, source_schema, &replay["source_domains"]);
    let db = native(&serde_json::json!({
        "target_before": replay["target_initial"],
        "source": replay["source"],
        "pk": false
    }));

    for (index, (step, transition)) in steps.iter().zip(transitions).enumerate() {
        let before = state("target", &step["before"]);
        let after = state("target", &step["after"]);
        assert_eq!(
            sorted_rows(&native_state(&db, "target")),
            sorted_rows(&before),
            "step {index}: native prestate"
        );
        assert_eq!(
            sorted_rows(transition.before()),
            sorted_rows(&before),
            "step {index}: canonical prestate"
        );
        assert_domains(transition.before(), schema, &step["before_domains"]);
        assert_eq!(
            transition.affected_target_ordinals(),
            ordinals(&step["affected_ordinals"]),
            "step {index}: target preimage identities"
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
        assert!(
            transition.intermediate_states().is_empty(),
            "step {index}: no invented independent source from prior target"
        );
        db.execute_batch(step["sql"].as_str().unwrap()).unwrap();
        assert_eq!(
            sorted_rows(&native_state(&db, "target")),
            sorted_rows(&after),
            "step {index}: independent DuckDB target state"
        );
        assert_eq!(
            sorted_rows(transition.after()),
            sorted_rows(&after),
            "step {index}: constructed target state"
        );
        assert_domains(transition.after(), schema, &step["after_domains"]);
        assert_eq!(
            sorted_rows(&native_state(&db, "source")),
            sorted_rows(&source),
            "step {index}: source never changes"
        );
    }

    let terminal = state("target", &replay["terminal"]);
    assert_eq!(
        sorted_rows(transitions.last().unwrap().after()),
        sorted_rows(&terminal)
    );
    assert_domains(&terminal, schema, &replay["terminal_domains"]);
    assert_eq!(
        sorted_rows(
            final_states
                .iter()
                .find(|s| s.relation() == "target")
                .unwrap()
        ),
        sorted_rows(&terminal)
    );
    assert_eq!(
        sorted_rows(
            final_states
                .iter()
                .find(|s| s.relation() == "source")
                .unwrap()
        ),
        sorted_rows(&source)
    );
}
