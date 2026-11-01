//! TASK-155: independent PK/UNIQUE/FK MERGE physical state and native DuckDB oracles.
//! These are additional mandatory cells beyond the TASK-153/154 matched branches.

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

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-155-acceptance-freeze.json")).unwrap()
}

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-155-fk").unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn schema() -> RelationCatalog {
    RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "target",
            ["id", "score"]
                .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", "duckdb").unwrap())
                .to_vec(),
        )
        .unwrap(),
        RelationSchema::new(
            "source",
            ["id", "score"]
                .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", "duckdb").unwrap())
                .to_vec(),
        )
        .unwrap(),
        RelationSchema::new(
            "parent",
            vec![SchemaColumn::from_sql_type("id", "INTEGER", "duckdb").unwrap()],
        )
        .unwrap(),
    ])
    .unwrap()
}

fn bundle(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let statement = SqlInput::inline(case["sql"].as_str().unwrap());
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = ConfiguredSqlInput::new("task155-fk", &statement, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[input], &schema()).unwrap();
    let mut constraints = Vec::new();
    let mut target = Vec::new();
    if case["target_pk"] == true {
        target.push(RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap());
    }
    if case["target_unique"] == true {
        target.push(RelationConstraint::unique_key(vec!["id".into()], evidence()).unwrap());
    }
    if case["score_fk"] == true {
        target.push(
            RelationConstraint::foreign_key(
                vec!["score".into()],
                "parent",
                vec!["id".into()],
                evidence(),
            )
            .unwrap(),
        );
    }
    if !target.is_empty() {
        constraints.push(RelationConstraintSet::new("target", target).unwrap());
    }
    if case["parent_pk"] == true {
        constraints.push(
            RelationConstraintSet::new(
                "parent",
                vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
            )
            .unwrap(),
        );
    }
    bundle.enrich_relation_constraints(&constraints);
    bundle
}

fn values(row: &Value, parent: bool) -> PhysicalWriteRow {
    let mut values = BTreeMap::new();
    for name in if parent {
        &["id"][..]
    } else {
        &["id", "score"][..]
    } {
        let literal = row[name]
            .as_i64()
            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()));
        values.insert((*name).to_string(), literal);
    }
    PhysicalWriteRow::new(values).unwrap()
}

fn state(name: &str, rows: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        name,
        rows.as_array()
            .unwrap()
            .iter()
            .map(|row| values(row, name == "parent"))
            .collect(),
    )
    .unwrap()
}

fn sorted(state: &PhysicalWriteState) -> Vec<String> {
    let mut values = state
        .rows()
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn assert_typed_domains(state: &PhysicalWriteState, relation: &RelationSchema, expected: &Value) {
    for domain in state.exact_domains(relation).unwrap() {
        let want = &expected[domain.column()];
        assert_eq!(want["type"], "INTEGER");
        let values = domain
            .values()
            .iter()
            .map(|(value, count)| {
                let key = match value {
                    LiteralValue::Number(n) => n.clone(),
                    LiteralValue::Null => "NULL".into(),
                    other => panic!("unexpected SQL INTEGER {other:?}"),
                };
                (key, *count)
            })
            .collect::<BTreeMap<_, _>>();
        let frozen_values = want["frequencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(value, count)| (value.clone(), count.as_u64().unwrap()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(values, frozen_values, "{} frequencies", domain.column());
        let numbers = values
            .keys()
            .filter(|value| value.as_str() != "NULL")
            .map(|n| n.parse::<i64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(numbers.iter().min().copied(), want["lower"].as_i64());
        assert_eq!(numbers.iter().max().copied(), want["upper"].as_i64());
        assert_eq!(values.contains_key("NULL"), want["nullable"]);
        assert_eq!(!numbers.is_empty(), want["lower_inclusive"]);
        assert_eq!(!numbers.is_empty(), want["upper_inclusive"]);
    }
}

fn native(case: &Value) -> Connection {
    let db = Connection::open_in_memory().unwrap();
    let key = if case["target_pk"] == true {
        " PRIMARY KEY"
    } else if case["target_unique"] == true {
        " UNIQUE"
    } else {
        ""
    };
    let fk = if case["score_fk"] == true {
        " REFERENCES parent(id)"
    } else {
        ""
    };
    db.execute_batch(&format!(
        "CREATE TABLE parent(id INTEGER PRIMARY KEY);
         CREATE TABLE target(id INTEGER{key}, score INTEGER{fk});
         CREATE TABLE source(id INTEGER,score INTEGER);"
    ))
    .unwrap();
    if case["parent"].is_array() {
        for row in case["parent"].as_array().unwrap() {
            db.execute(
                "INSERT INTO parent VALUES (?1)",
                params![row["id"].as_i64()],
            )
            .unwrap();
        }
    }
    for row in case["target_before"].as_array().unwrap() {
        db.execute(
            "INSERT INTO target(id,score) VALUES (?1,?2)",
            params![row["id"].as_i64(), row["score"].as_i64()],
        )
        .unwrap();
    }
    for row in case["source"].as_array().unwrap() {
        db.execute(
            "INSERT INTO source(id,score) VALUES (?1,?2)",
            params![row["id"].as_i64(), row["score"].as_i64()],
        )
        .unwrap();
    }
    db
}

fn native_state(db: &Connection, relation: &str) -> PhysicalWriteState {
    let columns = if relation == "parent" {
        "id"
    } else {
        "id,score"
    };
    let mut stmt = db
        .prepare(&format!("SELECT {columns} FROM {relation}"))
        .unwrap();
    let rows = stmt
        .query_map([], |r| {
            let id = r.get::<_, Option<i64>>(0)?;
            let score = if relation == "parent" {
                None
            } else {
                r.get::<_, Option<i64>>(1)?
            };
            Ok((id, score))
        })
        .unwrap()
        .map(Result::unwrap)
        .map(|(id, score)| {
            let mut values = BTreeMap::from([(
                "id".to_owned(),
                id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
            )]);
            if relation != "parent" {
                values.insert(
                    "score".to_owned(),
                    score.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                );
            }
            PhysicalWriteRow::new(values).unwrap()
        })
        .collect::<Vec<_>>();
    PhysicalWriteState::new(relation, rows).unwrap()
}

// Independent SQL EXISTS truth is computed over the *entire* opposing
// population, never by reimplementing the interpreter's match iterator.
fn native_ordinals(db: &Connection, relation: &str, not_true: bool) -> Vec<usize> {
    let opposite = if relation == "target" {
        "source"
    } else {
        "target"
    };
    let predicate = if not_true { "NOT EXISTS" } else { "EXISTS" };
    let sql = format!(
        "SELECT ordinal FROM (
          SELECT row_number() OVER (ORDER BY rowid)-1 AS ordinal,id FROM {relation}
        ) q WHERE {predicate} (SELECT 1 FROM {opposite} p WHERE q.id=p.id)
        ORDER BY ordinal"
    );
    let mut stmt = db.prepare(&sql).unwrap();
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
fn fk_and_unique_cases_preserve_complete_sources_and_do_not_guess_native_laws() {
    let freeze = frozen();
    assert_eq!(freeze["version"], 2);
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    for case in cases.iter().skip(7) {
        let id = case["id"].as_str().unwrap();
        let bundle = bundle(case);
        let schemas = bundle.source_schemas();
        let schema_of = |relation: &str| {
            schemas
                .iter()
                .find(|schema| schema.relation() == relation)
                .unwrap()
        };
        let before = state("target", &case["target_before"]);
        let source = state("source", &case["source"]);
        let parent = case["parent"]
            .as_array()
            .map(|_| state("parent", &case["parent"]));
        let mut initial = vec![before.clone(), source.clone()];
        if let Some(parent) = &parent {
            initial.push(parent.clone());
        }
        assert_typed_domains(&before, schema_of("target"), &case["before_domains"]);
        assert_typed_domains(&source, schema_of("source"), &case["source_domains"]);
        if let Some(parent) = &parent {
            assert_typed_domains(parent, schema_of("parent"), &case["parent_domains"]);
        }
        let plan = physical_ordered_write_state_plan(&bundle, &initial);
        match case["classification"].as_str().unwrap() {
            "Exact" => {
                let PhysicalWriteStatePlan::Feasible {
                    transitions,
                    final_states,
                } = plan
                else {
                    panic!("{id}: expected exact parent-key or UNIQUE construction: {plan:?}");
                };
                let expected = state("target", &case["target_after"]);
                assert_eq!(transitions.len(), 1);
                let step = &transitions[0];
                assert_eq!(sorted(step.before()), sorted(&before));
                assert_eq!(sorted(step.after()), sorted(&expected));
                assert_eq!(
                    step.affected().updated(),
                    case["affected"]["updated"].as_u64().unwrap()
                );
                assert_eq!(
                    step.affected().inserted(),
                    case["affected"]["inserted"].as_u64().unwrap()
                );
                assert_eq!(step.affected().deleted(), 0);
                assert!(step.intermediate_states().is_empty());
                assert_typed_domains(step.after(), schema_of("target"), &case["after_domains"]);
                assert_eq!(
                    final_states.iter().find(|s| s.relation() == "source"),
                    Some(&source)
                );
                if let Some(parent) = &parent {
                    assert_eq!(
                        final_states.iter().find(|s| s.relation() == "parent"),
                        Some(parent)
                    );
                }
                let db = native(case);
                assert_eq!(sorted(&native_state(&db, "target")), sorted(&before));
                assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                if let Some(parent) = &parent {
                    assert_eq!(sorted(&native_state(&db, "parent")), sorted(parent));
                }
                assert_eq!(
                    native_ordinals(&db, "target", false),
                    ordinals(&case["on_true_target_ordinals"])
                );
                assert_eq!(
                    native_ordinals(&db, "target", true),
                    ordinals(&case["on_not_true_target_ordinals"])
                );
                assert_eq!(
                    native_ordinals(&db, "source", false),
                    ordinals(&case["on_true_source_ordinals"])
                );
                assert_eq!(
                    native_ordinals(&db, "source", true),
                    ordinals(&case["on_not_true_source_ordinals"])
                );
                db.execute_batch(case["sql"].as_str().unwrap()).unwrap();
                assert_eq!(sorted(&native_state(&db, "target")), sorted(&expected));
                assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                if let Some(parent) = &parent {
                    assert_eq!(sorted(&native_state(&db, "parent")), sorted(parent));
                }
            }
            "Impossible" => {
                assert!(
                    matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
                    "{id}: FK violation must be Impossible, got {plan:?}"
                );
                let db = native(case);
                assert_eq!(sorted(&native_state(&db, "target")), sorted(&before));
                assert!(db.execute_batch(case["sql"].as_str().unwrap()).is_err());
                assert_eq!(sorted(&native_state(&db, "target")), sorted(&before));
                assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                if let Some(parent) = &parent {
                    assert_eq!(sorted(&native_state(&db, "parent")), sorted(parent));
                }
            }
            "Residual" => {
                let PhysicalWriteStatePlan::Residual { gap, .. } = plan else {
                    panic!("{id}: unsafe native-law claim: {plan:?}");
                };
                let expected = case["gap"].as_str().unwrap();
                assert_eq!(format!("{gap:?}"), expected, "{id}");
                if id == "82-C155-N-fk-missing-population" {
                    assert_eq!(gap, PhysicalWriteStateGap::MissingPopulation);
                    assert!(parent.is_none());
                } else {
                    assert_eq!(gap, PhysicalWriteStateGap::UnprovedConstraint);
                    let db = native(case);
                    // DuckDB's result is an observation, not permission to
                    // promote its nullable UNIQUE law to all SQL dialects.
                    let _ = db.execute_batch(case["sql"].as_str().unwrap());
                    assert_eq!(sorted(&native_state(&db, "source")), sorted(&source));
                }
            }
            other => panic!("{id}: unexpected proof class {other}"),
        }
    }
}
