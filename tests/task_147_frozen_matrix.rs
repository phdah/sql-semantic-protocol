//! Frozen TASK-147 full-bag acceptance oracles independent of parser heuristics.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan, RelationCatalog, RelationConstraint, RelationConstraintSet,
    RelationSchema, SchemaColumn, SqlInput,
};

fn value() -> serde_json::Value {
    serde_json::from_str(include_str!("../docs/task-147-acceptance-freeze.json")).unwrap()
}

fn rows(value: &serde_json::Value) -> Vec<(Option<i64>, i64)> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["id"].as_i64(), row["score"].as_i64().unwrap()))
        .collect()
}

fn state(name: &str, rows: &[(Option<i64>, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        name,
        rows.iter()
            .map(|(id, score)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".to_string(),
                        id.map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                    ),
                    ("score".to_string(), LiteralValue::Number(score.to_string())),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn bundle(sql: &str, key: bool) -> sql_semantic_protocol::AnalysisBundle {
    let schemas = ["source", "target"].map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "score"]
                .map(|column| SchemaColumn::from_sql_type(column, "INTEGER", "duckdb").unwrap())
                .to_vec(),
        )
        .unwrap()
    });
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let configured = ConfiguredSqlInput::new("task-147-frozen", &input, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], &catalog).unwrap();
    if key {
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::primary_key(
                vec!["id".into()],
                vec![ConstraintEvidence::new(
                    ConstraintProvenance::new(
                        ConstraintSourceKind::ExternalMetadata,
                        "task-147-frozen",
                    )
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

fn native_rows(db: &Connection, sql: &str) -> Vec<(Option<i64>, i64)> {
    let mut statement = db.prepare(sql).unwrap();
    let mut result = statement
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .unwrap()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn sorted(mut rows: Vec<(Option<i64>, i64)>) -> Vec<(Option<i64>, i64)> {
    rows.sort();
    rows
}

#[test]
fn every_frozen_mandatory_case_proves_full_source_target_returning_and_domain_oracle() {
    let matrix = value();
    let cases = matrix["mandatory_cases"].as_array().unwrap();
    assert_eq!(
        matrix["fixture_ids"],
        serde_json::json!(["80-C147-P", "80-C147-N"])
    );
    assert_eq!(cases.len(), 5);
    for case in cases {
        let sql = case["sql"].as_str().unwrap();
        let source = rows(&case["source"]);
        let prior = rows(&case["prestate"]);
        let has_key = case["id"] != "80-C147-N-unattested-target-key";
        let canonical = bundle(sql, has_key);
        let physical = physical_ordered_write_state_plan(
            &canonical,
            &[state("source", &source), state("target", &prior)],
        );
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE source(id INTEGER, score INTEGER);")
            .unwrap();
        if has_key {
            db.execute_batch("CREATE TABLE target(id INTEGER PRIMARY KEY,score INTEGER);")
                .unwrap();
        } else {
            db.execute_batch("CREATE TABLE target(id INTEGER,score INTEGER);")
                .unwrap();
        }
        for (id, score) in &source {
            db.execute(
                "INSERT INTO source VALUES (?1, ?2)",
                duckdb::params![id, score],
            )
            .unwrap();
        }
        for (id, score) in &prior {
            db.execute(
                "INSERT INTO target VALUES (?1, ?2)",
                duckdb::params![id, score],
            )
            .unwrap();
        }
        assert_eq!(
            native_rows(&db, "SELECT id,score FROM source"),
            sorted(source.clone())
        );
        assert_eq!(
            native_rows(&db, "SELECT id,score FROM source WHERE TRUE"),
            sorted(source.clone())
        );
        if let Some(source_sql) = case["source_select_sql"].as_str() {
            let selected = rows(&case["expected_selected_source_rows"]);
            assert_eq!(native_rows(&db, source_sql), sorted(selected));
            let excluded = native_rows(
                &db,
                "SELECT id,score FROM source WHERE NOT (id > 1) OR (id > 1) IS NULL",
            );
            assert_eq!(excluded, sorted(vec![(None, 40), (Some(1), 20)]));
            assert_eq!(
                native_rows(&db, "SELECT id,score FROM source WHERE (id > 1) IS NULL"),
                vec![(None, 40)],
            );
            assert_eq!(
                case["sql_not_true_source_ids"],
                serde_json::json!(["source#0", "source#1"]),
            );
            assert_eq!(
                case["expected_unknown_source_ids"],
                serde_json::json!(["source#0"])
            );
        }
        assert_eq!(
            native_rows(&db, "SELECT id,score FROM target"),
            sorted(prior.clone())
        );
        match case["proof"].as_str().unwrap() {
            "Exact" => {
                let PhysicalWriteStatePlan::Feasible {
                    transitions,
                    final_states,
                } = physical
                else {
                    panic!("{} should be exact: {physical:?}", case["id"]);
                };
                assert_eq!(transitions.len(), 1);
                let transition = &transitions[0];
                assert_eq!(
                    sorted(source.clone()),
                    native_rows(&db, "SELECT id,score FROM source")
                );
                assert_eq!(transition.before().rows(), state("target", &prior).rows());
                assert!(transition.intermediate_states().is_empty());
                let expected = rows(&case["expected_poststate"]);
                assert_eq!(transition.after().rows(), state("target", &expected).rows());
                let expected_returning = case["expected_returning"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| row["id"].as_i64().unwrap())
                    .collect::<Vec<_>>();
                let actual_returning = transition
                    .returning()
                    .unwrap()
                    .rows()
                    .iter()
                    .map(|row| match row.values().get("id").unwrap() {
                        LiteralValue::Number(v) => v.parse::<i64>().unwrap(),
                        other => panic!("unproven RETURNING value: {other:?}"),
                    })
                    .collect::<Vec<_>>();
                assert_eq!(actual_returning, expected_returning);
                assert_eq!(
                    transition.affected().inserted(),
                    case["expected_affected"]["inserted"].as_u64().unwrap()
                );
                assert_eq!(transition.affected().updated(), 0);
                assert_eq!(transition.affected().deleted(), 0);
                assert_eq!(
                    final_states
                        .iter()
                        .find(|s| s.relation() == "source")
                        .unwrap()
                        .rows(),
                    state("source", &source).rows()
                );
                let schema = canonical
                    .source_schemas()
                    .iter()
                    .find(|s| s.relation() == "target")
                    .unwrap();
                let domains = transition.after().exact_domains(schema).unwrap();
                let returning_domains = transition.returning_exact_domains(schema).unwrap();
                assert_eq!(returning_domains.len(), 1);
                assert_eq!(returning_domains[0].column(), "id");
                let returned_values = returning_domains[0].values();
                assert_eq!(returned_values.len(), 1);
                assert_eq!(returned_values[0].0, LiteralValue::Number(
                    expected_returning[0].to_string(),
                ));
                assert_eq!(returned_values[0].1, 1);
                for domain in domains {
                    let expected_domain = &case["typed_domains"][domain.column()];
                    assert_eq!(expected_domain["type"], "INTEGER");
                    let mut actual = domain
                        .values()
                        .iter()
                        .map(|(v, times)| {
                            (
                                match v {
                                    LiteralValue::Number(n) => n.parse::<i64>().unwrap(),
                                    other => panic!("unproved integer value: {other:?}"),
                                },
                                *times,
                            )
                        })
                        .collect::<Vec<_>>();
                    actual.sort();
                    assert!(!actual.is_empty());
                    assert_eq!(
                        actual.first().unwrap().0,
                        expected_domain["lower"].as_i64().unwrap()
                    );
                    assert_eq!(
                        actual.last().unwrap().0,
                        expected_domain["upper"].as_i64().unwrap()
                    );
                    assert_eq!(expected_domain["lower_inclusive"], true);
                    assert_eq!(expected_domain["upper_inclusive"], true);
                    assert_eq!(expected_domain["nullable"], false);
                    assert_eq!(
                        actual.iter().map(|(_, n)| *n).sum::<u64>(),
                        expected.len() as u64
                    );
                }
                let mut stmt = db.prepare(sql).unwrap();
                let mut observed_returning = stmt
                    .query_map([], |r| r.get::<_, i64>(0))
                    .unwrap()
                    .map(|r| r.unwrap())
                    .collect::<Vec<_>>();
                observed_returning.sort();
                assert_eq!(observed_returning, expected_returning);
                drop(stmt);
                assert_eq!(
                    native_rows(&db, "SELECT id,score FROM target"),
                    sorted(expected)
                );
                assert_eq!(
                    native_rows(&db, "SELECT id,score FROM source"),
                    sorted(source)
                );
            }
            "Residual" | "Impossible" => {
                if case["proof"] == "Residual" {
                    assert!(
                        matches!(physical, PhysicalWriteStatePlan::Residual { .. }),
                        "{}",
                        case["id"]
                    );
                } else {
                    assert!(
                        matches!(physical, PhysicalWriteStatePlan::Impossible { .. }),
                        "{}",
                        case["id"]
                    );
                }
                assert!(
                    db.execute_batch(sql).is_err(),
                    "invalid native case must reject: {}",
                    case["id"]
                );
                assert_eq!(
                    native_rows(&db, "SELECT id,score FROM target"),
                    sorted(prior)
                );
                assert_eq!(
                    native_rows(&db, "SELECT id,score FROM source"),
                    sorted(source)
                );
            }
            other => panic!("unknown frozen proof strength: {other}"),
        }
    }
}
