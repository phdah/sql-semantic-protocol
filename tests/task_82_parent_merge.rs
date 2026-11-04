//! TASK-82 independent parent crosswalk: mixed MERGE full-bag and fail-closed proof.
//! Expected rows and domains are frozen independently of the canonical interpreter.

use std::collections::BTreeMap;

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStateGap,
    PhysicalWriteStatePlan, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!(
        "../docs/task-82-parent-acceptance-freeze.json"
    ))
    .unwrap()
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

fn state(relation: &str, rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(relation, rows.as_array().unwrap().iter().map(row).collect()).unwrap()
}

fn sorted_rows(state: &PhysicalWriteState) -> Vec<String> {
    let mut rows = state
        .rows()
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(
        &["source", "target"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
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

fn analyze(sql: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let inputs = sql
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let dialect = dialect_from_name("duckdb").unwrap();
    let ids = (0..inputs.len())
        .map(|index| format!("task82-parent-step-{index}"))
        .collect::<Vec<_>>();
    let configured = inputs
        .iter()
        .zip(&ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, &catalog()).unwrap()
}

fn native(case: &Value) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE target(id INTEGER, score INTEGER);
         CREATE TABLE source(id INTEGER, score INTEGER);",
    )
    .unwrap();
    for relation in ["target", "source"] {
        for value in case["initial"][relation].as_array().unwrap() {
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
    assert!(["source", "target"].contains(&relation));
    let mut statement = db
        .prepare(&format!("SELECT id, score FROM {relation}"))
        .unwrap();
    let rows = statement
        .query_map([], |record| {
            Ok((
                record.get::<_, Option<i64>>(0)?,
                record.get::<_, Option<i64>>(1)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .map(|(id, score)| {
            PhysicalWriteRow::new(BTreeMap::from([
                (
                    "id".to_owned(),
                    id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                ),
                (
                    "score".to_owned(),
                    score.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                ),
            ]))
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new(relation, rows).unwrap()
}

fn native_match_count(db: &Connection, relation: &str, opposite: &str, not_true: bool) -> u64 {
    assert!(["source", "target"].contains(&relation));
    assert!(["source", "target"].contains(&opposite));
    let membership = if not_true { "NOT EXISTS" } else { "EXISTS" };
    let sql = format!(
        "SELECT count(*) FROM {relation} AS l WHERE {membership} \
         (SELECT 1 FROM {opposite} AS r WHERE l.id = r.id)"
    );
    let count: i64 = db.query_row(&sql, [], |row| row.get(0)).unwrap();
    u64::try_from(count).unwrap()
}

fn assert_domains(state: &PhysicalWriteState, schema: &RelationSchema, frozen: &Value) {
    let domains = state.exact_domains(schema).unwrap();
    assert_eq!(domains.len(), 2);
    for domain in domains {
        let expected = &frozen[domain.column()];
        let frequencies = domain
            .values()
            .iter()
            .map(|(value, count)| {
                (
                    match value {
                        LiteralValue::Null => "NULL".to_owned(),
                        LiteralValue::Number(number) => number.clone(),
                        other => panic!("unexpected scalar in INTEGER domain: {other:?}"),
                    },
                    *count,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let expected_frequencies = expected["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(number, count)| (number.clone(), count.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(frequencies, expected_frequencies, "{} bag", domain.column());
        let numeric = frequencies
            .keys()
            .filter(|number| number.as_str() != "NULL")
            .map(|number| number.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(numeric.iter().min().copied(), expected["lower"].as_i64());
        assert_eq!(numeric.iter().max().copied(), expected["upper"].as_i64());
        assert_eq!(
            frequencies.contains_key("NULL"),
            expected["nullable"] == true
        );
        assert_eq!(!numeric.is_empty(), expected["lower_inclusive"] == true);
        assert_eq!(!numeric.is_empty(), expected["upper_inclusive"] == true);
        assert_eq!(expected["type"], "INTEGER");
    }
}

#[test]
fn mixed_parent_merge_steps_preserve_complete_native_bags_and_domains() {
    let frozen = freeze();
    assert_eq!(frozen["version"], 1);
    let case = &frozen["cases"][0];
    assert_eq!(case["id"], "82-PARENT-MIXED-P");
    assert_eq!(case["classification"], "Exact");
    let steps = case["steps"].as_array().unwrap();
    let sql = steps
        .iter()
        .map(|step| step["sql"].as_str().unwrap())
        .collect::<Vec<_>>();
    let bundle = analyze(&sql);
    let initial = ["target", "source"]
        .into_iter()
        .map(|relation| state(relation, &case["initial"][relation]))
        .collect::<Vec<_>>();
    let plan = physical_ordered_write_state_plan(&bundle, &initial);
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = plan
    else {
        panic!("mixed ordered MERGE must be exact: {plan:?}");
    };
    assert_eq!(transitions.len(), 2);
    let schema = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "target")
        .unwrap();
    let db = native(case);

    for (step, transition) in steps.iter().zip(&transitions) {
        let before = state("target", &step["before"]);
        let after = state("target", &step["after"]);
        assert_eq!(sorted_rows(transition.before()), sorted_rows(&before));
        assert_eq!(
            sorted_rows(&native_state(&db, "target")),
            sorted_rows(&before)
        );
        assert_domains(transition.before(), schema, &step["before_domains"]);
        assert_domains(
            &native_state(&db, "target"),
            schema,
            &step["before_domains"],
        );
        assert_eq!(
            sorted_rows(&native_state(&db, "source")),
            sorted_rows(&state("source", &case["initial"]["source"]))
        );
        for (relation, opposite) in [("source", "target"), ("target", "source")] {
            let prefix = relation;
            assert_eq!(
                native_match_count(&db, relation, opposite, false),
                step["on"][format!("{prefix}_true")].as_u64().unwrap()
            );
            assert_eq!(
                native_match_count(&db, relation, opposite, true),
                step["on"][format!("{prefix}_not_true")].as_u64().unwrap()
            );
        }
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
        assert_eq!(sorted_rows(transition.after()), sorted_rows(&after));
        assert_domains(transition.after(), schema, &step["after_domains"]);
        db.execute_batch(step["sql"].as_str().unwrap()).unwrap();
        assert_eq!(
            sorted_rows(&native_state(&db, "target")),
            sorted_rows(&after)
        );
        assert_domains(&native_state(&db, "target"), schema, &step["after_domains"]);
    }

    for relation in ["source", "target"] {
        let expected = state(relation, &case["final"][relation]);
        let canonical = final_states
            .iter()
            .find(|state| state.relation() == relation)
            .unwrap();
        assert_eq!(sorted_rows(canonical), sorted_rows(&expected));
        assert_eq!(
            sorted_rows(&native_state(&db, relation)),
            sorted_rows(&expected)
        );
    }
}

#[test]
fn duplicate_source_match_cannot_be_accepted_as_a_portable_merge_winner() {
    let case = &freeze()["cases"][1];
    assert_eq!(case["id"], "82-PARENT-MULTIMATCH-N");
    assert_eq!(case["classification"], "Residual");
    assert_eq!(case["gap"], "UnprovedOperation");
    let bundle = analyze(&[case["sql"].as_str().unwrap()]);
    let snapshots = ["target", "source"]
        .into_iter()
        .map(|relation| state(relation, &case["initial"][relation]))
        .collect::<Vec<_>>();
    let plan = physical_ordered_write_state_plan(&bundle, &snapshots);
    assert!(
        matches!(
            plan,
            PhysicalWriteStatePlan::Residual {
                gap: PhysicalWriteStateGap::UnprovedOperation,
                ..
            }
        ),
        "repeated source matches must remain a typed residual: {plan:?}"
    );
    let db = native(case);
    for relation in ["target", "source"] {
        assert_eq!(
            sorted_rows(&native_state(&db, relation)),
            sorted_rows(&state(relation, &case["initial"][relation]))
        );
    }
    assert_eq!(
        native_match_count(&db, "source", "target", false),
        case["on"]["source_true"].as_u64().unwrap()
    );
    assert_eq!(
        native_match_count(&db, "target", "source", false),
        case["on"]["target_true"].as_u64().unwrap()
    );
}
