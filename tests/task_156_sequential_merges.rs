//! TASK-156 computed MERGE and ordered two-step physical-state verification.
//! All expected outcomes come from the frozen independent SQL row-state oracle.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStateGap, PhysicalWriteStatePlan, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-156-acceptance-freeze.json")).unwrap()
}

fn row(values: &Value) -> PhysicalWriteRow {
    PhysicalWriteRow::new(
        values
            .as_object()
            .unwrap()
            .iter()
            .map(|(column, value)| {
                (
                    column.clone(),
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

fn all_initial(case: &Value) -> Vec<PhysicalWriteState> {
    case["initial"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, rows)| state(name, rows))
        .collect()
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(
        &["source", "stage", "target", "terminal"]
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
    let inputs = case["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| SqlInput::inline(step["sql"].as_str().unwrap()))
        .collect::<Vec<_>>();
    let ids = (0..inputs.len())
        .map(|index| format!("task156-step-{index}"))
        .collect::<Vec<_>>();
    let dialect = dialect_from_name("duckdb").unwrap();
    let configured = inputs
        .iter()
        .zip(&ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap();
    if case["target_pk"] == true {
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::primary_key(
                vec!["id".to_owned()],
                vec![ConstraintEvidence::new(
                    ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task156")
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

fn sorted(state: &PhysicalWriteState) -> Vec<String> {
    let mut rows = state
        .rows()
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn native(case: &Value) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    let target_ddl = if case["target_pk"] == true {
        "CREATE TABLE target(id INTEGER PRIMARY KEY, score INTEGER);"
    } else {
        "CREATE TABLE target(id INTEGER, score INTEGER);"
    };
    db.execute_batch(target_ddl).unwrap();
    db.execute_batch(
        "CREATE TABLE source(id INTEGER, score INTEGER);
         CREATE TABLE stage(id INTEGER, score INTEGER);
         CREATE TABLE terminal(id INTEGER, score INTEGER);",
    )
    .unwrap();
    for (relation, rows) in case["initial"].as_object().unwrap() {
        assert!(
            ["source", "stage", "target", "terminal"].contains(&relation.as_str()),
            "only declared physical relations may be initialized"
        );
        for value in rows.as_array().unwrap() {
            db.execute(
                &format!("INSERT INTO {relation}(id, score) VALUES (?1, ?2)"),
                params![value["id"].as_i64(), value["score"].as_i64()],
            )
            .unwrap();
        }
    }
    db
}

fn native_state(db: &Connection, relation: &str) -> PhysicalWriteState {
    assert!(["source", "stage", "target", "terminal"].contains(&relation));
    let mut statement = db
        .prepare(&format!("SELECT id, score FROM {relation} ORDER BY rowid"))
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .map(|(id, score)| {
            PhysicalWriteRow::new(BTreeMap::from([
                (
                    "id".to_owned(),
                    id.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                ),
                (
                    "score".to_owned(),
                    score.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                ),
            ]))
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new(relation, rows).unwrap()
}

fn native_truth(db: &Connection, target: &str, source: &str, not_true: bool) -> Vec<usize> {
    assert!(["source", "stage", "target", "terminal"].contains(&target));
    assert!(["source", "stage", "target", "terminal"].contains(&source));
    let truth = if not_true { "NOT EXISTS" } else { "EXISTS" };
    let sql = format!(
        "SELECT ordinal FROM (
           SELECT row_number() OVER (ORDER BY rowid) - 1 AS ordinal, id FROM {target}
         ) AS t WHERE {truth} (SELECT 1 FROM {source} AS s WHERE t.id = s.id)
         ORDER BY ordinal"
    );
    let mut stmt = db.prepare(&sql).unwrap();
    stmt.query_map([], |row| row.get::<_, i64>(0))
        .unwrap()
        .map(Result::unwrap)
        .map(|value| usize::try_from(value).unwrap())
        .collect()
}

fn ordinals(values: &Value) -> Vec<usize> {
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|value| usize::try_from(value.as_u64().unwrap()).unwrap())
        .collect()
}

fn assert_domains(state: &PhysicalWriteState, schema: &RelationSchema, expected: &Value) {
    let domains = state.exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 2);
    for domain in domains {
        let column = &expected[domain.column()];
        let actual = domain
            .values()
            .iter()
            .map(|(value, count)| {
                (
                    match value {
                        LiteralValue::Null => "NULL".to_owned(),
                        LiteralValue::Number(number) => number.clone(),
                        other => panic!("unproved typed value {other:?}"),
                    },
                    *count,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let frozen = column["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(value, count)| (value.clone(), count.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual, frozen, "{} frequency bag", domain.column());
        let numbers = actual
            .keys()
            .filter(|value| value.as_str() != "NULL")
            .map(|value| value.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(numbers.iter().min().copied(), column["lower"].as_i64());
        assert_eq!(numbers.iter().max().copied(), column["upper"].as_i64());
        assert_eq!(
            actual.contains_key("NULL"),
            column["nullable"].as_bool().unwrap()
        );
        assert_eq!(
            !numbers.is_empty(),
            column["lower_inclusive"].as_bool().unwrap()
        );
        assert_eq!(
            !numbers.is_empty(),
            column["upper_inclusive"].as_bool().unwrap()
        );
        assert_eq!(column["type"], "INTEGER");
    }
}

#[test]
fn frozen_sequential_merge_snapshots_match_independent_duckdb() {
    let frozen = freeze();
    assert_eq!(frozen["version"], 2);
    for case in frozen["cases"].as_array().unwrap() {
        if case["classification"] != "Exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let bundle = analyze(case);
        let initial = all_initial(case);
        let plan = physical_ordered_write_state_plan(&bundle, &initial);
        let PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } = plan
        else {
            panic!("{id}: expected complete, exact sequential MERGE: {plan:?}");
        };
        let steps = case["steps"].as_array().unwrap();
        assert_eq!(transitions.len(), steps.len(), "{id}: statement count");
        let db = native(case);
        let mut expected_states = initial
            .iter()
            .map(|state| (state.relation().to_owned(), state.clone()))
            .collect::<BTreeMap<_, _>>();
        for (step, transition) in steps.iter().zip(&transitions) {
            let target = step["target"].as_str().unwrap();
            let source = step["source"].as_str().unwrap();
            let before = state(target, &step["before"]);
            let after = state(target, &step["after"]);
            let schema = bundle
                .source_schemas()
                .iter()
                .find(|schema| schema.relation() == target)
                .unwrap();
            let source_schema = bundle
                .source_schemas()
                .iter()
                .find(|schema| schema.relation() == source)
                .unwrap();
            assert_eq!(
                sorted(transition.before()),
                sorted(&before),
                "{id}: prestate"
            );
            assert_eq!(sorted(&native_state(&db, target)), sorted(&before));
            assert_domains(transition.before(), schema, &step["before_domains"]);
            assert_eq!(
                sorted(&native_state(&db, source)),
                sorted(expected_states.get(source).unwrap()),
                "{id}: physical source must reflect only prior committed writes"
            );
            assert_eq!(
                native_state(&db, source).exact_domains(source_schema),
                expected_states
                    .get(source)
                    .unwrap()
                    .exact_domains(source_schema)
            );
            assert_domains(
                &native_state(&db, source),
                source_schema,
                &step["source_before_domains"],
            );
            assert_eq!(
                native_truth(&db, source, target, false),
                ordinals(&step["on_true_source_ordinals"]),
                "{id}: ON TRUE source occurrence partition"
            );
            assert_eq!(
                native_truth(&db, source, target, true),
                ordinals(&step["on_not_true_source_ordinals"]),
                "{id}: ON NOT TRUE source occurrence partition"
            );
            assert_eq!(
                native_truth(&db, target, source, false),
                ordinals(&step["on_true_target_ordinals"]),
                "{id}: ON TRUE target occurrence partition"
            );
            assert_eq!(
                native_truth(&db, target, source, true),
                ordinals(&step["on_not_true_target_ordinals"]),
                "{id}: ON NOT TRUE target occurrence partition"
            );
            assert_eq!(
                transition.affected_target_ordinals(),
                ordinals(&step["affected_target_ordinals"]),
                "{id}: original target occurrence identity"
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
            assert!(transition.intermediate_states().is_empty());
            assert_eq!(
                sorted(transition.after()),
                sorted(&after),
                "{id}: poststate"
            );
            assert_domains(transition.after(), schema, &step["after_domains"]);
            db.execute_batch(step["sql"].as_str().unwrap()).unwrap();
            assert_eq!(
                sorted(&native_state(&db, target)),
                sorted(&after),
                "{id}: native after"
            );
            assert_domains(&native_state(&db, target), schema, &step["after_domains"]);
            expected_states.insert(target.to_owned(), after);
        }
        for (relation, expected) in case["final"].as_object().unwrap() {
            let actual = final_states
                .iter()
                .find(|state| state.relation() == relation)
                .unwrap();
            let schema = bundle
                .source_schemas()
                .iter()
                .find(|schema| schema.relation() == relation)
                .unwrap();
            assert_eq!(
                sorted(actual),
                sorted(&state(relation, expected)),
                "{id}: terminal bag"
            );
            assert_eq!(sorted(&native_state(&db, relation)), sorted(actual));
            assert_domains(actual, schema, &case["final_domains"][relation]);
        }
    }
}

#[test]
fn duplicate_merge_matches_remain_a_typed_residual() {
    let case = freeze()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "82-C156-N-duplicate-source")
        .unwrap()
        .clone();
    let plan = physical_ordered_write_state_plan(&analyze(&case), &all_initial(&case));
    assert!(
        matches!(
            plan,
            PhysicalWriteStatePlan::Residual {
                gap: PhysicalWriteStateGap::UnprovedOperation,
                ..
            }
        ),
        "duplicate source matches cannot select an arbitrary winner: {plan:?}"
    );
}

#[test]
fn computed_primary_key_collision_is_impossible_and_duckdb_atomic() {
    let frozen = freeze();
    let case = frozen["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "82-C156-N-computed-duplicate-primary-key")
        .unwrap();
    assert_eq!(case["classification"], "Impossible");
    let bundle = analyze(case);
    let original = all_initial(case);
    let plan = physical_ordered_write_state_plan(&bundle, &original);
    assert!(
        matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
        "an enforced computed primary-key collision cannot be feasible: {plan:?}"
    );
    let step = &case["steps"][0];
    let db = native(case);
    let target = state("target", &case["initial"]["target"]);
    let source = state("source", &case["initial"]["source"]);
    assert_eq!(sorted(&native_state(&db, "target")), sorted(&target));
    assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
    assert_eq!(
        native_truth(&db, "target", "source", false),
        ordinals(&step["on_true_target_ordinals"])
    );
    assert_eq!(
        native_truth(&db, "target", "source", true),
        ordinals(&step["on_not_true_target_ordinals"])
    );
    assert_eq!(
        native_truth(&db, "source", "target", false),
        ordinals(&step["on_true_source_ordinals"])
    );
    assert_eq!(
        native_truth(&db, "source", "target", true),
        ordinals(&step["on_not_true_source_ordinals"])
    );
    let target_schema = bundle
        .source_schemas()
        .iter()
        .find(|s| s.relation() == "target")
        .unwrap();
    assert_domains(&target, target_schema, &step["before_domains"]);
    assert!(
        db.execute_batch(step["sql"].as_str().unwrap()).is_err(),
        "native DuckDB must enforce the post-update PRIMARY KEY"
    );
    assert_eq!(sorted(&native_state(&db, "target")), sorted(&target));
    assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
}
