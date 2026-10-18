//! Public API verification of exact finite closed-world three-source bags.
//! DuckDB exercises the same immutable cases independently.
use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_chain_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalFiniteChainProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    ["l", "r", "t"]
        .into_iter()
        .map(|source| {
            let rows = case["physical_source_bags"][source]
                .as_array()
                .expect("frozen source bag")
                .iter()
                .map(|row| {
                    let values = row
                        .as_object()
                        .expect("fully named physical row")
                        .iter()
                        .map(|(name, value)| {
                            let literal = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(value) = value.as_i64() {
                                LiteralValue::Number(value.to_string())
                            } else {
                                LiteralValue::Text(
                                    value.as_str().expect("typed string identity").to_string(),
                                )
                            };
                            (name.to_string(), literal)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(values).expect("complete source row")
                })
                .collect();
            PhysicalWriteState::new(source, rows).expect("physical source identity")
        })
        .collect()
}

fn json_terminal(rows: Vec<BTreeMap<String, LiteralValue>>) -> Vec<Value> {
    let mut values = rows
        .into_iter()
        .map(|row| {
            Value::Object(
                row.into_iter()
                    .map(|(column, value)| {
                        let json = match value {
                            LiteralValue::Null => Value::Null,
                            LiteralValue::Number(value) => {
                                json!(value.parse::<i64>().expect("integer domain"))
                            }
                            LiteralValue::Text(value) => json!(value),
                            LiteralValue::Boolean(value) => json!(value),
                        };
                        (column, json)
                    })
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    values.sort_by_key(Value::to_string);
    values
}

fn frozen_triplets(case: &Value, kind: &str) -> Vec<[String; 3]> {
    let mut triples = case[kind]
        .as_array()
        .expect("complete P or NOT TRUE bag")
        .iter()
        .map(|triplet| {
            let components = triplet.as_array().expect("source identities");
            std::array::from_fn(|index| {
                components[index].as_str().expect("stable identity").to_string()
            })
        })
        .collect::<Vec<_>>();
    triples.sort();
    triples
}

#[test]
fn all_frozen_bags_match_normalized_sql_constructive_proof_in_thirteen_dialects() {
    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
            .expect("task-115 immutable cases");
    for case in frozen["cases"].as_array().expect("all cases") {
        let case_id = case["id"].as_str().expect("id");
        for dialect in frozen["dialects"].as_array().expect("thirteen dialects") {
            let dialect = dialect.as_str().expect("canonical dialect name");
            let source_schemas = ["l", "r", "t"]
                .into_iter()
                .map(|source| {
                    let columns = case["source_column_types"][source]
                        .as_object()
                        .expect("typed physical columns")
                        .iter()
                        .map(|(column, declaration)| {
                            let sql_type = declaration
                                .as_str()
                                .expect("typed column")
                                .split_whitespace()
                                .next()
                                .expect("SQL datatype");
                            SchemaColumn::from_sql_type(column, sql_type, "postgresql")
                                .expect("attested physical scalar type")
                        })
                        .collect();
                    RelationSchema::new(source, columns).expect("source schema")
                })
                .chain(std::iter::once(
                    RelationSchema::new(
                        "stage",
                        vec![
                            SchemaColumn::from_sql_type("k", "INTEGER", "postgresql")
                                .expect("stage key"),
                        ],
                    )
                    .expect("stage schema"),
                ))
                .collect::<Vec<_>>();
            let catalog = RelationCatalog::from_schemas(&source_schemas).expect("typed catalog");
            let sql = case["sql"].as_str().expect("canonical SQL");
            let statements = sql
                .split(';')
                .map(str::trim)
                .filter(|statement| !statement.is_empty())
                .map(SqlInput::inline)
                .collect::<Vec<_>>();
            let ids = (0..statements.len())
                .map(|index| format!("task-115-{case_id}-{index}"))
                .collect::<Vec<_>>();
            let parser = dialect_from_name(dialect).expect("supported dialect");
            let configured = statements
                .iter()
                .zip(&ids)
                .map(|(sql, id)| ConfiguredSqlInput::new(id, sql, dialect, parser.as_ref()))
                .collect::<Vec<_>>();
            let analyzed = analyze_configured_inputs_with_catalog(&configured, &catalog)
                .unwrap_or_else(|error| panic!("{case_id}/{dialect}: {error:?}"));
            let target = analyzed.layers().last().expect("frozen terminal layer");
            let proof = physical_finite_chain_bag_plan(
                &analyzed,
                target.id(),
                &snapshots(case),
            );
            let PhysicalFiniteChainProof::Exact {
                sources,
                filtered_true_ids,
                filtered_not_true_ids,
                mut selected,
                mut rejected,
                terminal,
                ..
            } = proof
            else {
                panic!("{case_id}/{dialect}: finite canonical proof failed: {proof:?}");
            };
            assert_eq!(sources, ["l", "r", "t"].map(str::to_string));
            selected.sort();
            rejected.sort();
            assert_eq!(
                selected,
                frozen_triplets(case, "selected_physical_triplets"),
                "{case_id}/{dialect}: SQL TRUE physical identity bag"
            );
            assert_eq!(
                rejected,
                frozen_triplets(case, "rejected_not_true_triplets"),
                "{case_id}/{dialect}: SQL NOT TRUE physical identity bag"
            );
            let mut actual_true = filtered_true_ids;
            let mut actual_not_true = filtered_not_true_ids;
            actual_true.sort();
            actual_not_true.sort();
            let frozen_ids = |name: &str| {
                let mut ids = case[name]
                    .as_array()
                    .expect("filter identity bag")
                    .iter()
                    .map(|v| v.as_str().expect("stable source identity").to_string())
                    .collect::<Vec<_>>();
                ids.sort();
                ids
            };
            assert_eq!(actual_true, frozen_ids("filtered_true_ids"), "{case_id}/{dialect}");
            assert_eq!(
                actual_not_true,
                frozen_ids("filtered_not_true_ids"),
                "{case_id}/{dialect}"
            );
            let grouped = case["variant"] == "downstream_group_count";
            let mut expected = case["expected_terminal_bag"]
                .as_array()
                .expect("terminal bag")
                .iter()
                .map(|row| {
                    if grouped {
                        json!({"k": row["k"], "n": row["n"]})
                    } else {
                        json!({"k": row["k"]})
                    }
                })
                .collect::<Vec<_>>();
            expected.sort_by_key(Value::to_string);
            assert_eq!(
                json_terminal(terminal),
                expected,
                "{case_id}/{dialect}: exact canonical terminal bag"
            );
        }
    }
}
