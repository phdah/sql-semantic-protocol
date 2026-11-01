//! TASK-153: complete physical MERGE matched UPDATE/DELETE bag and occurrence oracles.
//! Expected values are frozen before implementation in the task-owned matrix.

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
    serde_json::from_str(include_str!("../docs/task-153-acceptance-freeze.json")).unwrap()
}

fn row(value: &Value) -> PhysicalWriteRow {
    PhysicalWriteRow::new(
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    value.as_i64().map_or(LiteralValue::Null, |number| {
                        LiteralValue::Number(number.to_string())
                    }),
                )
            })
            .collect(),
    )
    .unwrap()
}

fn state(relation: &str, values: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        values.as_array().unwrap().iter().map(row).collect(),
    )
    .unwrap()
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(
        &["target", "source"]
            .into_iter()
            .map(|name| {
                RelationSchema::new(
                    name,
                    ["id", "score"]
                        .into_iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn analyze(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let text = SqlInput::inline(case["sql"].as_str().unwrap());
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = ConfiguredSqlInput::new("task153", &text, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog()).unwrap();
    if case["pk"] == true {
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::primary_key(
                vec!["id".to_owned()],
                vec![ConstraintEvidence::new(
                    ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-153")
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
    let mut result = vec![state("target", &case["target_before"])];
    if case["source"].is_array() {
        result.push(state("source", &case["source"]));
    }
    result
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
    for value in case["target_before"].as_array().unwrap() {
        db.execute(
            "INSERT INTO target(id,score) VALUES (?1,?2)",
            params![value["id"].as_i64(), value["score"].as_i64()],
        )
        .unwrap();
    }
    for value in case["source"].as_array().unwrap() {
        db.execute(
            "INSERT INTO source(id,score) VALUES (?1,?2)",
            params![value["id"].as_i64(), value["score"].as_i64()],
        )
        .unwrap();
    }
    db
}

fn native_state(db: &Connection, relation: &str) -> PhysicalWriteState {
    let order = if relation == "target" {
        "id NULLS FIRST"
    } else {
        "rowid"
    };
    let mut query = db
        .prepare(&format!("SELECT id,score FROM {relation} ORDER BY {order}"))
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

// Independent SQL EXISTS truth includes the entire source bag. NULL matches
// belong to NOT TRUE, never to a fabricated equality or selected occurrence.
fn native_match_ordinals(db: &Connection, not_true: bool) -> Vec<usize> {
    let operation = if not_true { "NOT EXISTS" } else { "EXISTS" };
    let statement = format!(
        "SELECT ordinal FROM (
           SELECT row_number() OVER (ORDER BY rowid) - 1 AS ordinal, id FROM target
         ) AS t WHERE {operation} (SELECT 1 FROM source AS s WHERE t.id = s.id)
         ORDER BY ordinal"
    );
    let mut query = db.prepare(&statement).unwrap();
    query
        .query_map([], |r| r.get::<_, i64>(0))
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

fn assert_domains(state: &PhysicalWriteState, schema: &RelationSchema, expected: &Value) {
    let actual = state.exact_domains(schema).unwrap();
    assert_eq!(actual.len(), 2);
    for column in actual {
        let expected_column = &expected[column.column()];
        let frequencies = column
            .values()
            .iter()
            .map(|(value, count)| {
                (
                    match value {
                        LiteralValue::Number(v) => v.clone(),
                        LiteralValue::Null => "NULL".to_string(),
                        other => panic!("unexpected typed MERGE scalar {other:?}"),
                    },
                    *count,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let frozen = expected_column["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, n)| (key.clone(), n.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(frequencies, frozen, "{} frequencies", column.column());
        let values = frequencies
            .keys()
            .filter(|key| key.as_str() != "NULL")
            .map(|key| key.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            values.iter().min().copied(),
            expected_column["lower"].as_i64()
        );
        assert_eq!(
            values.iter().max().copied(),
            expected_column["upper"].as_i64()
        );
        assert_eq!(
            frequencies.contains_key("NULL"),
            expected_column["nullable"].as_bool().unwrap()
        );
        assert_eq!(
            !values.is_empty(),
            expected_column["lower_inclusive"].as_bool().unwrap()
        );
        assert_eq!(
            !values.is_empty(),
            expected_column["upper_inclusive"].as_bool().unwrap()
        );
        assert_eq!(expected_column["type"], "INTEGER");
    }
}

#[test]
fn frozen_matched_merge_cases_match_complete_duckdb_bags_and_typed_bounds() {
    let frozen = freeze();
    assert_eq!(frozen["version"], 1);
    let cases = frozen["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    let exact = cases
        .iter()
        .filter(|case| case["proof"] == "Exact")
        .collect::<Vec<_>>();
    assert_eq!(exact.len(), 5);
    for case in exact {
        let id = case["id"].as_str().unwrap();
        let bundle = analyze(case);
        let schema = bundle
            .source_schemas()
            .iter()
            .find(|schema| schema.relation() == "target")
            .unwrap();
        let plan = physical_ordered_write_state_plan(&bundle, &snapshots(case));
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = &plan
        else {
            panic!("{id}: expected complete exact MERGE: {plan:?}");
        };
        assert_eq!(transitions.len(), 1, "{id}: one atomic action");
        let transition = &transitions[0];
        let db = native(case);
        let initial = state("target", &case["target_before"]);
        let source = state("source", &case["source"]);
        assert_eq!(
            native_state(&db, "target"),
            initial,
            "{id}: native target prestate"
        );
        assert_eq!(
            native_state(&db, "source"),
            source,
            "{id}: native source prestate"
        );
        assert_eq!(
            native_match_ordinals(&db, false),
            ordinals(&case["ordinals"])
        );
        assert_eq!(
            native_match_ordinals(&db, true),
            ordinals(&case["not_true_ordinals"])
        );
        assert_eq!(transition.before(), &initial, "{id}: canonical prestate");
        assert_domains(transition.before(), schema, &case["before_domains"]);
        assert_eq!(
            transition.affected_target_ordinals(),
            ordinals(&case["ordinals"]),
            "{id}: selected target row-occurrence identity"
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
        let expected_after = state("target", &case["target_after"]);
        assert_eq!(
            transition.after(),
            &expected_after,
            "{id}: canonical poststate"
        );
        assert_domains(transition.after(), schema, &case["after_domains"]);
        db.execute_batch(case["sql"].as_str().unwrap()).unwrap();
        let native_after = native_state(&db, "target");
        assert_eq!(
            native_after, expected_after,
            "{id}: independent DuckDB poststate"
        );
        assert_domains(&native_after, schema, &case["after_domains"]);
        assert_eq!(
            native_state(&db, "source"),
            source,
            "{id}: untouched source"
        );
        assert_eq!(
            final_states
                .iter()
                .find(|state| state.relation() == "target"),
            Some(&expected_after),
            "{id}: final target"
        );
    }
}

#[test]
fn frozen_merge_negatives_never_claim_exact_source_winners_or_invalid_final_rows() {
    let frozen = freeze();
    for case in frozen["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["proof"] != "Exact")
    {
        let id = case["id"].as_str().unwrap();
        let plan = physical_ordered_write_state_plan(&analyze(case), &snapshots(case));
        match case["proof"].as_str().unwrap() {
            "Residual" => {
                let PhysicalWriteStatePlan::Residual { gap, .. } = plan else {
                    panic!("{id}: unproved source must remain Residual: {plan:?}");
                };
                assert_eq!(format!("{gap:?}"), case["gap"].as_str().unwrap(), "{id}");
                if case["source"].is_array() {
                    let db = native(case);
                    // Native error behavior does not establish portable semantics.
                    let _ = db.execute_batch(case["sql"].as_str().unwrap());
                }
            }
            "Impossible" => {
                assert!(
                    matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
                    "{id}: {plan:?}"
                );
                let db = native(case);
                assert!(
                    db.execute_batch(case["sql"].as_str().unwrap()).is_err(),
                    "{id}: native key collision"
                );
                assert_eq!(
                    native_state(&db, "target"),
                    state("target", &case["target_before"])
                );
            }
            other => panic!("{id}: unexpected proof claim {other}"),
        }
    }
}
