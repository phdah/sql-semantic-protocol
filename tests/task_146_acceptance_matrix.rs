//! TASK-146 independent complete physical bag and typed-domain acceptance matrix.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    CheckCompareOperator, ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog, RelationConstraint,
    RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn matrix() -> serde_json::Value {
    serde_json::from_str(include_str!("../docs/task-146-acceptance-freeze.json")).unwrap()
}

fn rows(value: &serde_json::Value) -> Vec<(Option<i64>, Option<i64>)> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["id"].as_i64(), row["score"].as_i64()))
        .collect()
}

fn state(name: &str, input: &[(Option<i64>, Option<i64>)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        name,
        input
            .iter()
            .map(|(id, score)| {
                let scalar = |value: Option<i64>| {
                    value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
                };
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".to_string(), scalar(*id)),
                    ("score".to_string(), scalar(*score)),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-146-oracle")
            .unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn bundle(case: &serde_json::Value) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["source", "target"].map(|name| {
        RelationSchema::new(
            name,
            ["id", "score"]
                .into_iter()
                .map(|c| SchemaColumn::from_sql_type(c, "INTEGER", "postgresql").unwrap())
                .collect(),
        )
        .unwrap()
    });
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let dialect = dialect_from_name("duckdb").unwrap();
    let config = ConfiguredSqlInput::new("task-146-oracle", &sql, "duckdb", dialect.as_ref());
    let mut analysis = analyze_configured_inputs_with_catalog(&[config], &catalog).unwrap();
    let id = case["id"].as_str().unwrap();
    let mut target_constraints = Vec::new();
    let mut extra = Vec::new();
    if id.ends_with("pk-collision") {
        target_constraints
            .push(RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap());
    }
    if id.ends_with("check") {
        target_constraints.push(
            RelationConstraint::check_comparison(
                "score",
                CheckCompareOperator::Gte,
                ConstraintValue::Integer(0),
                evidence(),
            )
            .unwrap(),
        );
    }
    if id.ends_with("fk-safe") || id.ends_with("fk-violation") {
        target_constraints.push(
            RelationConstraint::foreign_key(
                vec!["score".to_string()],
                "source",
                vec!["id".to_string()],
                evidence(),
            )
            .unwrap(),
        );
        extra.push(
            RelationConstraintSet::new(
                "source",
                vec![RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap()],
            )
            .unwrap(),
        );
    }
    extra.push(RelationConstraintSet::new("target", target_constraints).unwrap());
    analysis.enrich_relation_constraints(&extra);
    analysis
}

fn literal_sql(value: Option<i64>) -> String {
    value.map_or("NULL".to_string(), |n| n.to_string())
}

fn duckdb_state(db: &Connection, name: &str) -> Vec<(Option<i64>, Option<i64>)> {
    let mut stmt = db
        .prepare(&format!(
            "SELECT id, score FROM {name} ORDER BY id NULLS FIRST, score NULLS FIRST"
        ))
        .unwrap();
    stmt.query_map([], |r| {
        Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?))
    })
    .unwrap()
    .map(|r| r.unwrap())
    .collect()
}

fn normalized_bag(mut bag: Vec<(Option<i64>, Option<i64>)>) -> Vec<(Option<i64>, Option<i64>)> {
    bag.sort();
    bag
}

#[test]
fn all_frozen_mandatory_cases_agree_with_native_duckdb_and_complete_domains() {
    let freeze = matrix();
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["80-C146-P", "80-C146-N"])
    );
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 13);
    assert_eq!(
        cases
            .iter()
            .filter(|c| c["classification"] == "mandatory")
            .count(),
        10
    );
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let source = case["source_row_bag"]
            .as_array()
            .map(|_| rows(&case["source_row_bag"]));
        let initial = rows(&case["prior_target_bag"]);
        let mut snapshots = vec![state("target", &initial)];
        if let Some(source) = &source {
            snapshots.push(state("source", source));
        }
        let analysis = bundle(case);
        let result = physical_ordered_write_state_plan(&analysis, &snapshots);
        let proof = case["proof"].as_str().unwrap();
        match (proof, &result) {
            (
                "Exact",
                PhysicalWriteStatePlan::Feasible {
                    transitions,
                    final_states,
                },
            ) => {
                let expected = rows(&case["expected_target_bag"]);
                let after = final_states
                    .iter()
                    .find(|s| s.relation() == "target")
                    .unwrap();
                assert_eq!(
                    after.rows(),
                    state("target", &expected).rows(),
                    "{id}: entire ordered target bag"
                );
                assert_eq!(
                    transitions[0].before().rows(),
                    state("target", &initial).rows(),
                    "{id}: prior target"
                );
                assert_eq!(
                    transitions[0].affected().inserted() as usize,
                    if id.ends_with("ordered") {
                        2
                    } else {
                        expected.len() - initial.len()
                    },
                    "{id}: affected count"
                );
                assert_eq!(
                    final_states.len(),
                    if source.is_some() { 2 } else { 1 },
                    "{id}: no invented physical relation"
                );
                if let Some(source) = &source {
                    assert_eq!(
                        final_states
                            .iter()
                            .find(|s| s.relation() == "source")
                            .unwrap()
                            .rows(),
                        state("source", source).rows(),
                        "{id}: untouched source"
                    );
                }
                if let Some(source_rows) = &source {
                    if let (Some(selected), Some(rejected)) = (
                        case["selected_source_rows"].as_array(),
                        case["rejected_source_rows"].as_array(),
                    ) {
                        let index = |label: &serde_json::Value| -> usize {
                            label
                                .as_str()
                                .unwrap()
                                .split('#')
                                .nth(1)
                                .unwrap()
                                .parse()
                                .unwrap()
                        };
                        let selected_indices = selected.iter().map(index).collect::<Vec<_>>();
                        let rejected_indices = rejected.iter().map(index).collect::<Vec<_>>();
                        let mut combined = selected_indices.clone();
                        combined.extend(rejected_indices);
                        combined.sort();
                        assert_eq!(
                            combined,
                            (0..source_rows.len()).collect::<Vec<_>>(),
                            "{id}: every physical source row classified as TRUE or NOT TRUE"
                        );
                        let selected_rows = selected_indices
                            .iter()
                            .map(|i| source_rows[*i])
                            .collect::<Vec<_>>();
                        let selected_bag = normalized_bag(selected_rows);
                        let appended = normalized_bag(
                            expected[initial.len()..(initial.len() + selected.len())].to_vec(),
                        );
                        assert_eq!(
                            selected_bag, appended,
                            "{id}: selected physical row identity and exact output bag"
                        );
                    }
                }
                assert!(
                    !expected.contains(&(Some(999999), Some(999999))),
                    "{id}: closed-world terminal negative tuple witness"
                );
                if let Some(witness) = case.get("expected_intermediate") {
                    let stage = &transitions[0].intermediate_states()[0];
                    assert_eq!(
                        stage.relation(),
                        witness["name"].as_str().unwrap(),
                        "{id}: stage name"
                    );
                    assert_eq!(
                        stage.rows(),
                        state(stage.relation(), &rows(&witness["rows"])).rows(),
                        "{id}: entire CTE bag"
                    );
                    assert_eq!(transitions[0].intermediate_states().len(), 1);
                    let stage_schema = RelationSchema::new(
                        stage.relation(),
                        ["id", "score"]
                            .into_iter()
                            .map(|c| {
                                SchemaColumn::from_sql_type(c, "INTEGER", "postgresql").unwrap()
                            })
                            .collect(),
                    )
                    .unwrap();
                    let stage_domains = stage.exact_domains(&stage_schema).unwrap();
                    assert_eq!(stage_domains.len(), 2);
                    for domain in stage_domains {
                        assert_eq!(
                            domain.values().iter().map(|(_, n)| *n).sum::<u64>(),
                            stage.rows().len() as u64,
                            "{id}: closed CTE column-frequency bag"
                        );
                    }
                } else {
                    assert!(
                        transitions[0].intermediate_states().is_empty(),
                        "{id}: no CTE"
                    );
                }
                let schema = analysis
                    .source_schemas()
                    .iter()
                    .find(|s| s.relation() == "target")
                    .unwrap();
                let domains = after.exact_domains(schema).unwrap();
                for column in domains {
                    let frozen = &case["terminal_typed_domains"][column.column()];
                    let values = expected
                        .iter()
                        .map(|row| {
                            if column.column() == "id" {
                                row.0
                            } else {
                                row.1
                            }
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        frozen["nullable"].as_bool().unwrap(),
                        values.iter().any(|v| v.is_none()),
                        "{id}: nullable"
                    );
                    let some = values.iter().filter_map(|v| *v).collect::<Vec<_>>();
                    assert_eq!(
                        frozen["lower"].as_i64(),
                        some.iter().min().copied(),
                        "{id}: inclusive lower"
                    );
                    assert_eq!(
                        frozen["upper"].as_i64(),
                        some.iter().max().copied(),
                        "{id}: inclusive upper"
                    );
                    assert!(
                        frozen["lower_inclusive"].as_bool().unwrap()
                            && frozen["upper_inclusive"].as_bool().unwrap()
                    );
                    assert_eq!(
                        column.values().iter().map(|(_, n)| *n).sum::<u64>(),
                        expected.len() as u64,
                        "{id}: complete histogram"
                    );
                }
            }
            ("Impossible", PhysicalWriteStatePlan::Impossible { .. }) => {
                assert!(
                    case["expected_target_bag"].is_null(),
                    "{id}: impossible cannot fabricate poststate"
                );
            }
            ("Residual", PhysicalWriteStatePlan::Residual { .. }) => {
                assert_eq!(
                    case["classification"], "conditional",
                    "{id}: no mandatory residual"
                );
            }
            _ => panic!("{id}: expected {proof}, got {result:?}"),
        }

        // SQL execution is the independent source/target oracle, not a snapshot
        // derived from the physical evaluator.
        if case["classification"] != "mandatory" {
            continue;
        }
        let db = Connection::open_in_memory().unwrap();
        let source_ddl = if id.ends_with("fk-safe") || id.ends_with("fk-violation") {
            "CREATE TABLE source(id INTEGER PRIMARY KEY,score INTEGER);"
        } else {
            "CREATE TABLE source(id INTEGER,score INTEGER);"
        };
        let target_ddl = if id.ends_with("pk-collision") {
            "CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);"
        } else if id.ends_with("check") {
            "CREATE TABLE target(id INTEGER,score INTEGER CHECK(score >= 0));"
        } else if id.ends_with("fk-safe") || id.ends_with("fk-violation") {
            "CREATE TABLE target(id INTEGER,score INTEGER REFERENCES source(id));"
        } else {
            "CREATE TABLE target(id INTEGER,score INTEGER);"
        };
        db.execute_batch(source_ddl).unwrap();
        db.execute_batch(target_ddl).unwrap();
        if let Some(source) = &source {
            for (id, score) in source {
                db.execute_batch(&format!(
                    "INSERT INTO source VALUES ({},{})",
                    literal_sql(*id),
                    literal_sql(*score)
                ))
                .unwrap();
            }
        }
        for (id, score) in &initial {
            db.execute_batch(&format!(
                "INSERT INTO target VALUES ({},{})",
                literal_sql(*id),
                literal_sql(*score)
            ))
            .unwrap();
        }
        assert_eq!(
            duckdb_state(&db, "source"),
            normalized_bag(source.clone().unwrap_or_default()),
            "{id}: complete initial source"
        );
        assert_eq!(
            duckdb_state(&db, "target"),
            normalized_bag(initial.clone()),
            "{id}: initial target"
        );
        let outcome = db.execute_batch(case["sql"].as_str().unwrap());
        if proof == "Impossible" {
            assert!(outcome.is_err(), "{id}: DuckDB must reject");
            assert_eq!(
                duckdb_state(&db, "target"),
                normalized_bag(initial.clone()),
                "{id}: atomic rollback"
            );
        } else {
            outcome.unwrap_or_else(|e| panic!("{id}: native DuckDB failed: {e}"));
            assert_eq!(
                duckdb_state(&db, "target"),
                normalized_bag(rows(&case["expected_target_bag"])),
                "{id}: complete terminal DuckDB bag"
            );
        }
    }
}
