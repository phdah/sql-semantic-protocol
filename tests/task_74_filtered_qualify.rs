//! Parent TASK-74: original AC #3 complete WHERE -> window -> QUALIFY proofs.
//! Expected bags were frozen before the physical evaluator was extended.

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_window_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, ValueDomain,
};
use sqlparser::parser::Parser;
use std::collections::{BTreeMap, BTreeSet};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-74-filtered-qualify-freeze.json"))
        .expect("immutable filtered-QUALIFY oracle")
}

fn rows(case: &Value) -> Vec<PhysicalWriteRow> {
    case["physical_source_bags"]["s"]
        .as_array()
        .expect("complete source")
        .iter()
        .map(|row| {
            PhysicalWriteRow::new(
                row.as_object()
                    .expect("source tuple")
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.clone(),
                            value.as_i64().map_or(LiteralValue::Null, |number| {
                                LiteralValue::Number(number.to_string())
                            }),
                        )
                    })
                    .collect(),
            )
            .expect("physical row")
        })
        .collect()
}

fn catalog(dialect: &str) -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "s",
        ["id", "k", "x"]
            .into_iter()
            .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", dialect).unwrap())
            .collect(),
    )
    .unwrap()])
    .unwrap()
}

fn analyze(case: &Value, dialect: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name(dialect).unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            case["id"].as_str().unwrap(),
            &sql,
            dialect,
            parser.as_ref(),
        )],
        &catalog(dialect),
    )
    .unwrap()
}

fn source(case: &Value) -> PhysicalWriteState {
    PhysicalWriteState::new("s", rows(case)).unwrap()
}

fn ids(case: &Value, field: &str) -> Vec<String> {
    case[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect()
}

fn literal_bag(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["expected_full_terminal_bag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_object()
                .unwrap()
                .iter()
                .map(|(col, value)| {
                    (
                        col.to_owned(),
                        value.as_i64().map_or(LiteralValue::Null, |number| {
                            LiteralValue::Number(number.to_string())
                        }),
                    )
                })
                .collect()
        })
        .collect()
}

fn bag(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    result.sort();
    result
}

fn native(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE s(id INTEGER, k INTEGER, x INTEGER)")
        .unwrap();
    for row in case["physical_source_bags"]["s"].as_array().unwrap() {
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ({}, {}, {})",
            row["id"], row["k"], row["x"]
        ))
        .unwrap();
    }
    let mut source = db.prepare("SELECT id, k, x FROM s ORDER BY id").unwrap();
    let actual_source = source
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected_source = case["physical_source_bags"]["s"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["id"].as_i64().unwrap(),
                row["k"].as_i64().unwrap(),
                row["x"].as_i64(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(actual_source, expected_source, "DuckDB physical bag");

    let mut statement = db.prepare(case["sql"].as_str().unwrap()).unwrap();
    statement
        .query_map([], |row| {
            let mut values = BTreeMap::new();
            for index in 0..row.as_ref().column_count() {
                let name = row.as_ref().column_name(index)?;
                let value = row.get::<_, Option<i64>>(index)?;
                values.insert(
                    name.to_string(),
                    value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                );
            }
            Ok(values)
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[test]
fn exact_where_then_qualify_preserves_closed_world_three_valued_identities_and_domains() {
    let freeze = freeze();
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["mandatory_case_ids"].as_array().unwrap().len(), 4);
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if case["proof"] != "Exact" {
            continue;
        }
        let bundle = analyze(case, "duckdb");
        let target = bundle.layers().last().unwrap().id();
        let proof = physical_finite_window_bag_plan(&bundle, target, &[source(case)]);
        let PhysicalFiniteWindowProof::Exact(exact) = proof else {
            panic!("{id}: mandatory physical proof is not Exact: {proof:?}");
        };
        let all_ids = (0..6).map(|i| format!("s#{i}")).collect::<BTreeSet<_>>();
        let true_ids = ids(case, "where_true_ids")
            .into_iter()
            .collect::<BTreeSet<_>>();
        let false_ids = ids(case, "where_not_true_ids")
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            true_ids.union(&false_ids).cloned().collect::<BTreeSet<_>>(),
            all_ids,
            "{id}"
        );
        assert!(true_ids.is_disjoint(&false_ids), "{id}");
        assert_eq!(
            exact
                .source_row_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>(),
            all_ids,
            "{id}"
        );
        assert_eq!(
            exact.rejected_upstream_row_ids,
            ids(case, "where_not_true_ids"),
            "{id}"
        );
        assert_eq!(
            exact.rejected_row_ids,
            ids(case, "qualify_not_true_ids"),
            "{id}"
        );
        assert_eq!(
            exact.terminal_row_ids,
            ids(case, "terminal_row_ids"),
            "{id}"
        );
        assert_eq!(
            exact.terminal_row_ids.len() + exact.rejected_row_ids.len(),
            true_ids.len(),
            "{id}: QUALIFY must partition only WHERE TRUE rows"
        );
        assert_eq!(
            bag(&exact.terminal),
            bag(&literal_bag(case)),
            "{id}: frozen terminal bag"
        );
        assert_eq!(
            bag(&exact.terminal),
            bag(&native(case)),
            "{id}: independent DuckDB bag"
        );

        for (name, expectation) in case["typed_terminal_domains"].as_object().unwrap() {
            let actual = exact
                .terminal_domains
                .get(name)
                .expect("terminal column domain");
            if expectation["empty"] == true {
                assert!(matches!(actual, ValueDomain::Empty), "{id}/{name}");
                continue;
            }
            let ValueDomain::Set(values) = actual else {
                panic!("{id}/{name}: expected exact finite values: {actual:?}");
            };
            let actual_numbers = values
                .iter()
                .filter_map(|v| match v {
                    LiteralValue::Number(number) => number.parse::<i64>().ok(),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                actual_numbers.iter().next().copied(),
                expectation["inclusive_bounds"][0].as_i64(),
                "{id}/{name}: lower inclusive bound"
            );
            assert_eq!(
                actual_numbers.iter().next_back().copied(),
                expectation["inclusive_bounds"][1].as_i64(),
                "{id}/{name}: upper inclusive bound"
            );
            assert_eq!(
                values.contains(&LiteralValue::Null),
                expectation["nullable"] == true,
                "{id}/{name}"
            );
        }
    }
}

#[test]
fn computed_where_is_residual_even_if_a_complete_sample_is_available() {
    let freeze = freeze();
    let case = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "74-PARENT-FILTERED-C1")
        .unwrap();
    let bundle = analyze(case, "duckdb");
    let target = bundle.layers().last().unwrap().id();
    let proof = physical_finite_window_bag_plan(&bundle, target, &[source(case)]);
    assert!(
        matches!(proof, PhysicalFiniteWindowProof::Residual { .. }),
        "{proof:?}"
    );
}

#[test]
fn frozen_filtered_window_parser_matrix_covers_all_thirteen_dialects() {
    let freeze = freeze();
    let dialects = freeze["required_dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    for case in freeze["cases"].as_array().unwrap() {
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            let parser = dialect_from_name(name).unwrap();
            let expected = case["parser_status_by_dialect"][name].as_str().unwrap();
            let parsed = Parser::parse_sql(parser.as_ref(), case["sql"].as_str().unwrap()).is_ok();
            assert_eq!(parsed, expected == "parsed", "{name} / {}", case["id"]);
            if parsed && case["proof"] == "Exact" {
                let bundle = analyze(case, name);
                let target = bundle.layers().last().unwrap().id();
                let proof = physical_finite_window_bag_plan(&bundle, target, &[source(case)]);
                assert!(
                    matches!(proof, PhysicalFiniteWindowProof::Exact(_)),
                    "{name}: {proof:?}"
                );
            }
        }
    }
}
