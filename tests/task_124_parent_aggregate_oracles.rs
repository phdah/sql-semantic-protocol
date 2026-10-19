//! TASK-124: independent TASK-73 acceptance oracle at the public SQL/physical seam.
//! Expectations are frozen independently of the child test modules. DuckDB executes
//! complete physical inputs and full terminal outputs, not just row counts.
use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_aggregate_bag_plan,
    physical_finite_join_aggregate_plan, AnalysisBundle, ConfiguredSqlInput, LiteralValue,
    PhysicalAggregateTruth, PhysicalFiniteAggregateProof, PhysicalJoinedAggregateProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SetMode,
    SqlInput, ValueDomain,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-124-acceptance-freeze.json"))
        .expect("TASK-124 task-owned acceptance matrix")
}

fn scalar(value: &Value) -> LiteralValue {
    match value {
        Value::Null => LiteralValue::Null,
        Value::Number(number) => LiteralValue::Number(number.to_string()),
        Value::String(text) => LiteralValue::Text(text.clone()),
        _ => panic!("unsupported frozen scalar: {value}"),
    }
}

fn source_names(case: &Value) -> &'static [&'static str] {
    if case["scope"] == "joined" {
        &["l", "r"]
    } else {
        &["s"]
    }
}

fn source_rows<'a>(case: &'a Value, name: &str) -> &'a [Value] {
    if case["scope"] == "joined" {
        case["source_bags"][name]
            .as_array()
            .expect("closed joined source")
    } else {
        case["source_bag"]
            .as_array()
            .expect("closed aggregate source")
    }
}

fn state(case: &Value, name: &str) -> PhysicalWriteState {
    PhysicalWriteState::new(
        name,
        source_rows(case, name)
            .iter()
            .map(|row| {
                PhysicalWriteRow::new(
                    row.as_object()
                        .expect("source row")
                        .iter()
                        .map(|(key, val)| (key.clone(), scalar(val)))
                        .collect(),
                )
                .expect("typed source row")
            })
            .collect(),
    )
    .expect("complete physical relation")
}

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(
        &["s", "l", "r"]
            .into_iter()
            .map(|name| {
                RelationSchema::new(
                    name,
                    ["id", "k", "x"]
                        .into_iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(
                                column,
                                if column == "id" { "VARCHAR" } else { "INTEGER" },
                                "postgresql",
                            )
                            .expect("source type")
                        })
                        .collect(),
                )
                .expect("typed relation")
            })
            .collect::<Vec<_>>(),
    )
    .expect("typed catalog")
}

fn bundle(case: &Value, dialect: &str) -> Result<AnalysisBundle, String> {
    let parser = dialect_from_name(dialect).expect("exposed parser");
    let sql = SqlInput::inline(case["sql"].as_str().expect("SQL"));
    let inputs = [ConfiguredSqlInput::new(
        case["id"].as_str().expect("case ID"),
        &sql,
        dialect,
        parser.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&inputs, &catalog()).map_err(|error| error.to_string())
}

fn expected_rows(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    case["full_terminal_bag"]
        .as_array()
        .expect("full output bag")
        .iter()
        .map(|row| {
            row.as_object()
                .expect("output row")
                .iter()
                .map(|(name, val)| (name.clone(), scalar(val)))
                .collect()
        })
        .collect()
}

fn normalized(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut values = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn check_domains(case: &Value, actual: &BTreeMap<String, ValueDomain>) {
    for expected in case["typed_terminal_domains"]
        .as_array()
        .expect("typed domains")
    {
        let name = expected["column"].as_str().expect("outcome column");
        let observed = actual
            .get(name)
            .unwrap_or_else(|| panic!("{name}: missing domain"));
        let mut desired = expected["values"]
            .as_array()
            .expect("exact values")
            .iter()
            .map(|value| format!("{:?}", scalar(value)))
            .collect::<Vec<_>>();
        desired.sort();
        if desired.is_empty() {
            assert!(
                matches!(observed, ValueDomain::Empty),
                "{name}: unbounded empty set"
            );
            continue;
        }
        let ValueDomain::Set(set) = observed else {
            panic!("{name}: outcome must be exact included set: {observed:?}");
        };
        assert_eq!(
            set.mode(),
            SetMode::Include,
            "{name}: inclusion, not exclusion"
        );
        let mut values = set
            .values()
            .iter()
            .map(|value| format!("{:?}", value.value()))
            .collect::<Vec<_>>();
        values.sort();
        assert_eq!(values, desired, "{name}: exact finite values and NULLs");
        assert_eq!(
            observed.admits_null(),
            expected["contains_null"].as_bool(),
            "{name}: NULL"
        );
        let mut numbers = expected["values"]
            .as_array()
            .expect("values")
            .iter()
            .filter_map(Value::as_i64)
            .collect::<Vec<_>>();
        numbers.sort();
        if let (Some(lower), Some(upper)) = (numbers.first(), numbers.last()) {
            let interval = &expected["inclusive_bounds"];
            assert_eq!(interval["lower"].as_i64(), Some(*lower), "{name}: lower");
            assert_eq!(interval["upper"].as_i64(), Some(*upper), "{name}: upper");
            assert_eq!(interval["lower_inclusive"], true, "{name}: inclusive lower");
            assert_eq!(interval["upper_inclusive"], true, "{name}: inclusive upper");
        }
    }
}

fn check_group_evidence(
    expected: &Value,
    actual_rows: &[String],
    actual_contributors: &BTreeMap<String, Vec<String>>,
    actual_truth: Option<PhysicalAggregateTruth>,
) {
    let wanted_truth = match expected["having"].as_str() {
        Some("TRUE") => Some(PhysicalAggregateTruth::True),
        Some("FALSE") => Some(PhysicalAggregateTruth::False),
        Some("UNKNOWN") => Some(PhysicalAggregateTruth::Unknown),
        None => None,
        other => panic!("invalid group truth: {other:?}"),
    };
    assert_eq!(
        actual_truth, wanted_truth,
        "SQL three-valued HAVING group truth"
    );
    let wanted_rows = expected["source_row_ids"]
        .as_array()
        .expect("group source IDs")
        .iter()
        .map(|v| v.as_str().expect("row ID").to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        actual_rows, wanted_rows,
        "all physical contributing and rejected source rows"
    );
    let wanted_contributors = expected["aggregate_row_ids"]
        .as_object()
        .expect("per-aggregate identities")
        .iter()
        .map(|(name, rows)| {
            (
                name.clone(),
                rows.as_array()
                    .expect("aggregate identities")
                    .iter()
                    .map(|v| v.as_str().expect("source identity").to_string())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual_contributors, &wanted_contributors,
        "multiplicity and NULL exclusions in every aggregate's actual contributors"
    );
}

fn duckdb(case: &Value) -> Vec<BTreeMap<String, LiteralValue>> {
    let db = Connection::open_in_memory().expect("independent DuckDB engine");
    for name in source_names(case) {
        db.execute_batch(&format!(
            "CREATE TABLE {name}(id VARCHAR, k INTEGER, x INTEGER)"
        ))
        .expect("source schema");
        for row in source_rows(case, name) {
            let sql_int = |column: &str| {
                row[column]
                    .as_i64()
                    .map_or_else(|| "NULL".to_string(), |n| n.to_string())
            };
            db.execute_batch(&format!(
                "INSERT INTO {name} VALUES ('{}', {}, {})",
                row["id"].as_str().expect("row identity"),
                sql_int("k"),
                sql_int("x"),
            ))
            .expect("full source insertion");
        }
        let mut physical = db
            .prepare(&format!("SELECT id, k, x FROM {name}"))
            .expect("source readback");
        let observed = physical
            .query_map([], |row| {
                Ok(BTreeMap::from([
                    (
                        "id".to_string(),
                        LiteralValue::Text(row.get::<_, String>(0)?),
                    ),
                    (
                        "k".to_string(),
                        row.get::<_, Option<i64>>(1)?
                            .map(|n| LiteralValue::Number(n.to_string()))
                            .unwrap_or(LiteralValue::Null),
                    ),
                    (
                        "x".to_string(),
                        row.get::<_, Option<i64>>(2)?
                            .map(|n| LiteralValue::Number(n.to_string()))
                            .unwrap_or(LiteralValue::Null),
                    ),
                ]))
            })
            .expect("source query")
            .collect::<Result<Vec<_>, _>>()
            .expect("source bag");
        let expected = source_rows(case, name)
            .iter()
            .map(|row| {
                row.as_object()
                    .expect("source row")
                    .iter()
                    .map(|(column, val)| (column.clone(), scalar(val)))
                    .collect()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            normalized(&observed),
            normalized(&expected),
            "{name}: complete source IDs"
        );
    }
    let columns = case["terminal_columns"]
        .as_array()
        .expect("terminal columns");
    let mut query = db
        .prepare(case["sql"].as_str().expect("SQL"))
        .expect("executable full terminal SQL");
    query
        .query_map([], |row| {
            let mut result = BTreeMap::new();
            for (index, name) in columns.iter().enumerate() {
                let name = name.as_str().expect("named terminal column");
                let value = if name == "average" {
                    row.get::<_, Option<f64>>(index)?
                        .map(|n| {
                            assert!(
                                n.is_finite() && n.fract() == 0.0,
                                "nonintegral AVG needs typed residual"
                            );
                            LiteralValue::Number((n as i64).to_string())
                        })
                        .unwrap_or(LiteralValue::Null)
                } else {
                    row.get::<_, Option<i64>>(index)?
                        .map(|n| LiteralValue::Number(n.to_string()))
                        .unwrap_or(LiteralValue::Null)
                };
                result.insert(name.to_string(), value);
            }
            Ok(result)
        })
        .expect("terminal rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("terminal bag")
}

#[test]
fn frozen_scope_covers_six_aggregates_grouping_filter_joins_and_negative_oracles() {
    let doc = freeze();
    assert_eq!(
        doc["fixture_ids"],
        serde_json::json!(["73-C124-P", "73-C124-N"])
    );
    assert_eq!(
        doc["required_dialects"].as_array().expect("dialects").len(),
        13
    );
    assert_eq!(
        doc["required_adapters"].as_array().expect("adapters").len(),
        3
    );
    let cases = doc["cases"].as_array().expect("frozen cases");
    assert_eq!(
        cases
            .iter()
            .filter(|case| case["proof_kind"] == "mandatory")
            .count(),
        12
    );
    assert_eq!(
        cases
            .iter()
            .filter(|case| case["proof_kind"] == "conditional")
            .count(),
        4
    );
    for task in ["TASK-121", "TASK-122", "TASK-123"] {
        assert!(
            cases.iter().any(|case| case["source_task"] == task),
            "{task}: missing evidence"
        );
    }
    for case in cases {
        assert!(case["id"].as_str().expect("ID").starts_with("73-C124-"));
        if case["proof_kind"] == "mandatory" {
            assert!(case["sql"].is_string());
            assert!(
                case["typed_terminal_domains"].is_array(),
                "typed outcomes required"
            );
            assert!(case["full_terminal_bag"].is_array(), "full oracle required");
            assert!(
                case["proof"].is_object(),
                "positive/NOT TRUE proof required"
            );
        } else {
            assert_eq!(case["owner"], "TASK-124");
            assert_eq!(case["expected_proof"], "Residual_or_parser_boundary");
        }
    }
    let expected = &doc["conformance_dbt"];
    assert_eq!(expected["expected_rows"], serde_json::json!(["north:1:42"]));
    let sql = include_str!("fixtures/conformance/v1/dbt_core_project/models/aggregate_summary.sql");
    assert!(sql.contains("{{ ref('enriched_orders') }}"));
    assert!(sql.contains("having count(*) >= 1"));
}

#[test]
fn independent_full_duckdb_bags_and_canonical_domains_certify_all_mandatory_cases() {
    for case in freeze()["cases"].as_array().expect("cases") {
        if case["proof_kind"] != "mandatory" {
            continue;
        }
        let id = case["id"].as_str().expect("case");
        let bundle = bundle(case, "generic").unwrap_or_else(|error| panic!("{id}: {error}"));
        let target = bundle.layers().last().expect("output").id();
        let states = source_names(case)
            .iter()
            .map(|name| state(case, name))
            .collect::<Vec<_>>();
        if case["scope"] == "joined" {
            let proof = physical_finite_join_aggregate_plan(&bundle, target, &states);
            let PhysicalJoinedAggregateProof::Exact(exact) = proof else {
                panic!("{id}: joined source completeness not proven: {proof:?}");
            };
            assert_eq!(
                normalized(&exact.terminal),
                normalized(&expected_rows(case)),
                "{id}: canonical"
            );
            check_domains(case, &exact.terminal_domains);

            if let Some(expected_groups) = case["groups"].as_array() {
                assert_eq!(
                    exact.groups.len(),
                    expected_groups.len(),
                    "{id}: every group classified"
                );
                for (got, wanted) in exact.groups.iter().zip(expected_groups) {
                    check_group_evidence(
                        wanted,
                        &got.source_row_ids,
                        &got.aggregate_row_ids,
                        got.having_truth,
                    );
                }
            }

            let pair_list = |value: &Value| {
                value
                    .as_array()
                    .expect("pair rows")
                    .iter()
                    .map(|v| {
                        [
                            v[0].as_str().expect("left").to_string(),
                            v[1].as_str().expect("right").to_string(),
                        ]
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                exact.selected,
                pair_list(&case["full_selected_pairs"]),
                "{id}: TRUE physical pairs"
            );
            assert_eq!(
                exact.rejected,
                pair_list(&case["full_rejected_pairs"]),
                "{id}: NOT TRUE physical pairs"
            );
        } else {
            let proof = physical_finite_aggregate_bag_plan(&bundle, target, &states);
            let PhysicalFiniteAggregateProof::Exact(exact) = proof else {
                panic!("{id}: single-source closure not proven: {proof:?}");
            };
            assert_eq!(
                normalized(&exact.terminal),
                normalized(&expected_rows(case)),
                "{id}: canonical"
            );
            check_domains(case, &exact.terminal_domains);

            if let Some(expected_groups) = case["groups"].as_array() {
                assert_eq!(
                    exact.groups.len(),
                    expected_groups.len(),
                    "{id}: every group classified"
                );
                for (got, wanted) in exact.groups.iter().zip(expected_groups) {
                    check_group_evidence(
                        wanted,
                        &got.source_row_ids,
                        &got.aggregate_row_ids,
                        got.having_truth,
                    );
                }
            }

            assert_eq!(exact.source, "s", "{id}: physical source identity");
        }
        assert_eq!(
            normalized(&duckdb(case)),
            normalized(&expected_rows(case)),
            "{id}: independent complete DuckDB terminal bag"
        );
    }
}

#[test]
fn thirteen_parser_dialects_keep_mandatory_exactness_and_grouping_extensions_fail_closed() {
    let matrix = freeze();
    for dialect in matrix["required_dialects"].as_array().expect("13 dialects") {
        let dialect = dialect.as_str().expect("dialect name");
        for case in matrix["cases"].as_array().expect("cases") {
            let parsed = bundle(case, dialect);
            let id = case["id"].as_str().expect("case");
            let expected = &case["expected_parser_dialect_cells"][dialect];
            assert_eq!(
                expected["owner"], "TASK-124",
                "{dialect}:{id}: conditional owner"
            );
            let expected_parser = expected["parser"].as_str().expect("frozen parser law");
            let expected_canonical = expected["canonical"].as_str().expect("frozen outcome");
            assert!(matches!(
                expected_parser,
                "must_parse" | "parse_or_reject" | "may_parse_or_reject"
            ));
            assert!(matches!(
                expected_canonical,
                "Exact_if_parsed" | "Residual_if_parsed"
            ));
            let Ok(bundle) = parsed else {
                assert_ne!(
                    expected_parser, "must_parse",
                    "{dialect}:{id}: unexpected mandatory parser boundary: {parsed:?}"
                );
                continue;
            };
            let target = bundle.layers().last().expect("target").id();
            let states = source_names(case)
                .iter()
                .map(|name| state(case, name))
                .collect::<Vec<_>>();
            let exact = if case["scope"] == "joined" {
                match physical_finite_join_aggregate_plan(&bundle, target, &states) {
                    PhysicalJoinedAggregateProof::Exact(_) => true,
                    PhysicalJoinedAggregateProof::Residual { .. } => false,
                    other => {
                        panic!("{dialect}:{id}: impossible is not a parser boundary: {other:?}")
                    }
                }
            } else {
                match physical_finite_aggregate_bag_plan(&bundle, target, &states) {
                    PhysicalFiniteAggregateProof::Exact(_) => true,
                    PhysicalFiniteAggregateProof::Residual { .. } => false,
                    other => {
                        panic!("{dialect}:{id}: impossible is not a parser boundary: {other:?}")
                    }
                }
            };
            assert_eq!(
                exact,
                expected_canonical == "Exact_if_parsed",
                "{dialect}:{id}: frozen exact mandatory or residual vendor syntax"
            );
        }
    }
}

#[test]
fn impossible_duplicate_physical_identity_is_not_confused_with_missing_source_evidence() {
    let doc = freeze();
    let case = doc["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["source_fixture"] == "73-C121-P-global")
        .expect("single-source baseline");
    let bundle = bundle(case, "generic").expect("SQL");
    let target = bundle.layers().last().expect("target").id();
    let state = state(case, "s");
    assert!(matches!(
        physical_finite_aggregate_bag_plan(&bundle, target, &[state.clone(), state]),
        PhysicalFiniteAggregateProof::Impossible { .. },
    ));
    assert!(matches!(
        physical_finite_aggregate_bag_plan(&bundle, target, &[]),
        PhysicalFiniteAggregateProof::Residual { .. },
    ));
}
