//! TASK-73 parent acceptance: independent original-AC and evidence-preservation guard.
//!
//! Executable complete-bag semantics are owned by the merged TASK-121..124
//! public Rust/DuckDB/dbt/ODCS tests. This test pins the finite parent scope
//! independently so dropping a mandatory case or promoting a residual fails CI.

use serde_json::{json, Value};
use std::collections::BTreeSet;

fn freeze(raw: &str) -> Value {
    serde_json::from_str(raw).expect("versioned acceptance contract")
}

fn rows<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().expect("finite acceptance inventory")
}

fn text(value: &Value) -> bool {
    value.as_str().is_some_and(|value| !value.trim().is_empty())
}

struct AcRequirement(
    u64,
    &'static [&'static str],
    &'static [&'static str],
    &'static [&'static str],
);

const REQUIRED_CASES: &[&str] = &[
    "73-C124-P-1-1",
    "73-C124-P-1-2",
    "73-C124-P-1-3",
    "73-C124-N-1-4",
    "73-C124-P-1-5",
    "73-C124-P-2-1",
    "73-C124-P-2-2",
    "73-C124-N-2-3",
    "73-C124-P-2-4",
    "73-C124-P-3-1",
    "73-C124-N-3-2",
    "73-C124-P-3-3",
];
const CHILD_CONDITIONAL_GROUPINGS: &[&str] = &[
    "73-C124-N-3-4",
    "73-C124-N-3-5",
    "73-C124-N-3-6",
    "73-C124-N-3-7",
];
const CONDITIONAL_GROUPINGS: &[&str] = &[
    "73-C124-N-3-4",
    "73-C124-N-3-5",
    "73-C124-N-3-6",
    "73-C124-N-3-7",
    "73-C73-C-grouping-function",
];
const CONDITIONAL_NUMERIC: &[&str] = &[
    "73-C121-C-decimal-average",
    "73-C121-C-overflow",
    "73-C121-C-noninteger",
];

#[test]
fn original_aggregate_parent_acceptance_has_an_independent_fixed_scope() {
    let parent = freeze(include_str!("../docs/task-73-acceptance-freeze.json"));
    assert_eq!(parent["task"], "TASK-73");
    assert_eq!(parent["version"], 1);
    assert_eq!(parent["mandatory_case_ids"], json!(REQUIRED_CASES));
    assert_eq!(parent["conditional_case_ids"], json!(CONDITIONAL_GROUPINGS));
    assert_eq!(
        parent["fixture_ids"],
        json!([
            "73-C121-P",
            "73-C121-N",
            "73-C122-P",
            "73-C122-N",
            "73-C123-P",
            "73-C123-N",
            "73-C124-P",
            "73-C124-N"
        ])
    );
    let mappings = rows(&parent, "original_acceptance_criteria");
    let expected: &[AcRequirement] = &[
        AcRequirement(
            1,
            &[
                "73-C124-P-1-1",
                "73-C124-P-1-2",
                "73-C124-P-1-5",
                "73-C124-P-2-1",
                "73-C124-P-2-2",
                "73-C124-P-2-4",
            ],
            &[],
            &[],
        ),
        AcRequirement(
            2,
            &[
                "73-C124-N-2-3",
                "73-C124-P-3-1",
                "73-C124-N-3-2",
                "73-C124-P-3-3",
            ],
            &[],
            &[],
        ),
        AcRequirement(
            3,
            &["73-C124-P-1-3", "73-C124-N-1-4", "73-C124-P-3-3"],
            CONDITIONAL_GROUPINGS,
            &[],
        ),
        AcRequirement(
            4,
            &[
                "73-C124-P-1-2",
                "73-C124-P-1-5",
                "73-C124-N-2-3",
                "73-C124-P-3-1",
                "73-C124-N-3-2",
            ],
            &[],
            CONDITIONAL_NUMERIC,
        ),
        AcRequirement(
            5,
            &["73-C124-P-3-1", "73-C124-N-3-2", "73-C124-P-3-3"],
            &[],
            &[],
        ),
        AcRequirement(
            6,
            REQUIRED_CASES,
            CONDITIONAL_GROUPINGS,
            CONDITIONAL_NUMERIC,
        ),
    ];
    assert_eq!(mappings.len(), expected.len());
    for (criterion, AcRequirement(id, required, grouping, numeric)) in
        mappings.iter().zip(expected.iter())
    {
        assert_eq!(criterion["id"], *id);
        assert_eq!(
            criterion["mandatory_case_ids"],
            json!(required),
            "AC #{id} mandatory scope changed"
        );
        assert_eq!(
            criterion["conditional_case_ids"],
            json!(grouping),
            "AC #{id} grouping boundary changed"
        );
        assert_eq!(
            criterion["numeric_conditional_ids"],
            json!(numeric),
            "AC #{id} numeric boundary changed"
        );
        assert!(text(&criterion["source"]));
    }
}

#[test]
fn parent_evidence_has_exact_complete_source_terminal_bags_and_adapters() {
    let parent = freeze(include_str!("../docs/task-73-acceptance-freeze.json"));
    let parent_cases = freeze(include_str!("../docs/task-124-acceptance-freeze.json"));
    let child_121 = freeze(include_str!("../docs/task-121-acceptance-freeze.json"));
    let child_122 = freeze(include_str!("../docs/task-122-acceptance-freeze.json"));
    let child_123 = freeze(include_str!("../docs/task-123-acceptance-freeze.json"));
    let gate = &parent["portable_gate"];
    assert_eq!(gate["mandatory_complete_bag_cases"], 12);
    assert_eq!(gate["mandatory_parser_cells"], 156);
    assert_eq!(gate["mandatory_adapter_cells"], 36);
    assert_eq!(gate["conditional_grouping_cases"], 5);
    assert_eq!(gate["conditional_parser_cells"], 65);
    assert_eq!(gate["conditional_adapter_cells"], 15);
    assert_eq!(gate["engine"], "duckdb");
    assert_eq!(gate["native_vendor_engines_certified"], false);
    let dialects: BTreeSet<&str> = [
        "generic",
        "mysql",
        "postgresql",
        "hive",
        "sqlite",
        "snowflake",
        "redshift",
        "mssql",
        "clickhouse",
        "bigquery",
        "ansi",
        "duckdb",
        "databricks",
    ]
    .into_iter()
    .collect();
    let adapters: BTreeSet<&str> = ["direct_sql", "dbt_compiled", "odcs_schema"]
        .into_iter()
        .collect();
    let mut actual = BTreeSet::new();
    for case in rows(&parent_cases, "cases") {
        if case["proof_kind"] != "mandatory" {
            continue;
        }
        let id = case["id"].as_str().expect("mandatory case id");
        assert!(actual.insert(id), "duplicate parent oracle case {id}");
        assert!(text(&case["sql"]));
        assert_eq!(
            case["proof"]["terminal"], "Exact",
            "non-Exact mandatory case: {id}"
        );
        assert!(text(&case["proof"]["source"]));
        assert!(text(&case["proof"]["not_true"]));
        assert!(
            case["full_terminal_bag"].is_array(),
            "missing terminal snapshot: {id}"
        );
        assert!(
            case["source_bag"].is_array() || case["source_bags"].is_object(),
            "missing complete physical source snapshot: {id}"
        );
        assert!(
            case["typed_terminal_domains"].is_array(),
            "missing typed domain: {id}"
        );
        for domain in rows(case, "typed_terminal_domains") {
            assert!(text(&domain["column"]));
            assert!(domain["values"].is_array());
            assert!(domain["contains_null"].is_boolean());
            if let Some(bounds) = domain["inclusive_bounds"].as_object() {
                assert!(bounds["lower_inclusive"].is_boolean());
                assert!(bounds["upper_inclusive"].is_boolean());
            }
        }
        let source = match case["source_task"].as_str().expect("source child task") {
            "TASK-121" => &child_121,
            "TASK-122" => &child_122,
            "TASK-123" => &child_123,
            other => panic!("unreviewed child owner: {other}"),
        };
        assert!(
            rows(source, "cases")
                .iter()
                .any(|original| original["id"] == case["source_fixture"]),
            "parent oracle {id} has no original child fixture"
        );
        let actual_dialects: BTreeSet<&str> = case["expected_parser_dialect_cells"]
            .as_object()
            .expect("dialect matrix")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(actual_dialects, dialects, "incomplete dialect matrix: {id}");
        let actual_adapters: BTreeSet<&str> = case["expected_adapter_cells"]
            .as_object()
            .expect("adapter matrix")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(actual_adapters, adapters, "incomplete adapter matrix: {id}");
        for status in case["expected_adapter_cells"]
            .as_object()
            .expect("adapter cells")
            .values()
        {
            assert_eq!(
                status, "Exact_required",
                "uncertified mandatory adapter: {id}"
            );
        }
    }
    assert_eq!(actual, REQUIRED_CASES.iter().copied().collect());
}

#[test]
fn conditional_aggregate_laws_are_reviewed_without_exactness_promotion() {
    let parent = freeze(include_str!("../docs/task-73-acceptance-freeze.json"));
    let child_121 = freeze(include_str!("../docs/task-121-acceptance-freeze.json"));
    let child_124 = freeze(include_str!("../docs/task-124-acceptance-freeze.json"));
    assert_eq!(parent["portable_gate"]["conditional_grouping_cases"], 5);
    assert_eq!(parent["portable_gate"]["conditional_parser_cells"], 65);
    assert_eq!(parent["portable_gate"]["conditional_adapter_cells"], 15);
    let groupings = rows(&parent, "grouping_boundaries");
    assert_eq!(groupings.len(), CHILD_CONDITIONAL_GROUPINGS.len());
    for (boundary, expected_id) in groupings.iter().zip(CHILD_CONDITIONAL_GROUPINGS) {
        assert_eq!(boundary["case_id"], *expected_id);
        assert_eq!(boundary["owner"], "TASK-124");
        assert!(text(&boundary["reason"]));
        assert!(text(&boundary["release_disposition"]));
        let case = rows(&child_124, "cases")
            .iter()
            .find(|case| case["id"] == *expected_id)
            .expect("conditional case remains frozen");
        assert_eq!(case["proof_kind"], "conditional");
        assert_eq!(case["owner"], "TASK-124");
        assert!(
            !case["full_terminal_bag"].is_array(),
            "conditional promoted to Exact"
        );
        assert!(text(&case["required_boundary"]));
        assert!(text(&case["conditional_release_disposition"]));
        assert_eq!(
            case["expected_parser_dialect_cells"]
                .as_object()
                .expect("dialect cells")
                .len(),
            13
        );
        assert_eq!(
            case["expected_adapter_cells"]
                .as_object()
                .expect("adapter cells")
                .len(),
            3
        );
    }
    let numeric = rows(&parent, "numeric_boundaries");
    assert_eq!(numeric.len(), CONDITIONAL_NUMERIC.len());
    for (boundary, expected_id) in numeric.iter().zip(CONDITIONAL_NUMERIC) {
        assert_eq!(boundary["id"], *expected_id);
        assert_eq!(boundary["owner"], "TASK-121");
        assert!(text(&boundary["reason"]));
        assert!(text(&boundary["release_disposition"]));
        let original = rows(&child_121, "conditional_boundaries")
            .iter()
            .find(|item| item["id"] == *expected_id)
            .expect("numeric boundary remains frozen");
        assert_eq!(original["owner"], "TASK-121");
        assert!(text(&original["reason"]));
        assert!(matches!(
            original["release_disposition"].as_str(),
            Some("not_certified_no_exactness_claim" | "fail_closed_residual")
        ));
        assert_eq!(
            boundary["release_disposition"],
            "reviewed_residual_no_exactness_claim"
        );
    }
}

#[test]
fn materialized_aggregate_dbt_oracle_is_not_replaced_with_a_sql_simulation() {
    let parent = freeze(include_str!("../docs/task-73-acceptance-freeze.json"));
    let child = freeze(include_str!("../docs/task-124-acceptance-freeze.json"));
    assert_eq!(
        parent["portable_gate"]["materialized_dbt_oracle"],
        "tests/protocol_conformance_dbt_e2e.rs"
    );
    assert_eq!(
        child["conformance_dbt"]["model_path"],
        "tests/fixtures/conformance/v1/dbt_core_project/models/aggregate_summary.sql"
    );
    assert_eq!(
        child["conformance_dbt"]["expected_rows"],
        json!(["north:1:42"])
    );
    let model =
        include_str!("fixtures/conformance/v1/dbt_core_project/models/aggregate_summary.sql");
    assert!(
        model.contains("ref("),
        "dbt fixture must preserve its upstream model dependency"
    );
}

#[test]
fn parent_coverage_manifest_agrees_with_frozen_original_acceptance() {
    let parent = freeze(include_str!("../docs/task-73-acceptance-freeze.json"));
    let manifest = freeze(include_str!("../docs/coverage-manifest.json"));
    let row = &manifest["task_73_parent_acceptance"];
    assert_eq!(row["owner"], "TASK-73");
    assert_eq!(row["freeze"], "docs/task-73-acceptance-freeze.json");
    assert_eq!(row["original_ac"], json!([1, 2, 3, 4, 5, 6]));
    assert_eq!(
        row["mandatory_cases"],
        parent["portable_gate"]["mandatory_complete_bag_cases"]
    );
    assert_eq!(
        row["conditional_grouping_cases"],
        parent["portable_gate"]["conditional_grouping_cases"]
    );
    assert_eq!(row["numeric_conditional_boundaries"], 3);
    assert_eq!(
        row["mandatory_parser_cells"],
        parent["portable_gate"]["mandatory_parser_cells"]
    );
    assert_eq!(
        row["mandatory_adapter_cells"],
        parent["portable_gate"]["mandatory_adapter_cells"]
    );
    assert_eq!(
        row["reviewed_conditional_parser_cells"],
        parent["portable_gate"]["conditional_parser_cells"]
    );
    assert_eq!(
        row["reviewed_conditional_adapter_cells"],
        parent["portable_gate"]["conditional_adapter_cells"]
    );
    assert_eq!(
        row["grouping_function_case_id"],
        parent["grouping_function_boundary"]["id"]
    );
    assert_eq!(row["release_approved"], false);
    assert_eq!(row["parent_signed_off"], false);
    assert_eq!(row["verified_original_ac"], json!([1, 3, 4, 5, 6]));
    assert_eq!(
        parent["status"],
        "parent_portable_floor_verified_original_ac2_unresolved"
    );
    let ac2 = rows(&parent, "original_acceptance_criteria")
        .iter()
        .find(|criterion| criterion["id"] == 2)
        .expect("original AC #2");
    assert_eq!(
        ac2["verification_status"],
        "pending_parent_cross_task_oracle_ci"
    );
    assert_eq!(
        ac2["upstream_composition_case_ids"],
        json!(["71-C115-P-GROUP", "71-C115-N-GROUP"])
    );
}

#[test]
fn parent_grouping_function_distinguishes_null_keys_and_fails_closed() {
    use duckdb::Connection;
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, dialect_from_name,
        physical_finite_aggregate_bag_plan, ConfiguredSqlInput, LiteralValue,
        PhysicalFiniteAggregateProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
        RelationSchema, SchemaColumn, SqlInput,
    };
    use sqlparser::parser::Parser;

    let parent = freeze(include_str!("../docs/task-73-acceptance-freeze.json"));
    let child = freeze(include_str!("../docs/task-124-acceptance-freeze.json"));
    let boundary = &parent["grouping_function_boundary"];
    assert_eq!(boundary["id"], "73-C73-C-grouping-function");
    assert_eq!(boundary["owner"], "TASK-73");
    assert!(text(&boundary["reason"]));
    assert!(text(&boundary["release_disposition"]));

    let mut source_rows = Vec::new();
    for id in rows(boundary, "source_case_ids") {
        let source = rows(&child, "cases")
            .iter()
            .find(|case| case["id"] == *id)
            .expect("source fixture remains frozen");
        source_rows.extend(rows(source, "source_bag").iter().cloned());
    }

    let db = Connection::open_in_memory().expect("independent DuckDB oracle");
    db.execute_batch("CREATE TABLE s(id VARCHAR, k INTEGER, x INTEGER)")
        .expect("typed source schema");
    for row in &source_rows {
        let number = |key: &str| {
            row[key]
                .as_i64()
                .map_or_else(|| "NULL".to_string(), |n| n.to_string())
        };
        db.execute_batch(&format!(
            "INSERT INTO s VALUES ('{}', {}, {})",
            row["id"].as_str().expect("source row identity"),
            number("k"),
            number("x"),
        ))
        .expect("populate complete source bag");
    }

    let sql = boundary["sql"].as_str().expect("GROUPING syntax");
    let mut query = db.prepare(sql).expect("DuckDB supports GROUPING");
    let actual = query
        .query_map([], |row| {
            Ok(json!({
                "k": row.get::<_, Option<i64>>(0)?,
                "grouping_flag": row.get::<_, i64>(1)?,
                "row_count": row.get::<_, i64>(2)?,
            }))
        })
        .expect("execute terminal bag")
        .collect::<Result<Vec<_>, _>>()
        .expect("DuckDB terminal output");
    let canonical = |bag: &[Value]| {
        let mut rows = bag.iter().map(Value::to_string).collect::<Vec<_>>();
        rows.sort();
        rows
    };
    assert_eq!(
        canonical(&actual),
        canonical(rows(boundary, "expected_duckdb_terminal_bag")),
        "GROUPING must distinguish an ordinary NULL key from a subtotal NULL"
    );

    let expected_dialects: BTreeSet<&str> = parent["portable_gate"]["required_dialects"]
        .as_array()
        .expect("required dialects")
        .iter()
        .map(|v| v.as_str().expect("dialect"))
        .collect();
    let parser_cells = boundary["expected_parser_dialect_cells"]
        .as_object()
        .expect("parser classifications");
    let actual_dialects: BTreeSet<&str> = parser_cells.keys().map(String::as_str).collect();
    assert_eq!(actual_dialects, expected_dialects);
    let expected_adapters: BTreeSet<&str> = parent["portable_gate"]["required_adapters"]
        .as_array()
        .expect("required adapters")
        .iter()
        .map(|v| v.as_str().expect("adapter"))
        .collect();
    let adapter_cells = boundary["expected_adapter_cells"]
        .as_object()
        .expect("adapter classifications");
    let actual_adapters: BTreeSet<&str> = adapter_cells.keys().map(String::as_str).collect();
    assert_eq!(actual_adapters, expected_adapters);
    for status in adapter_cells.values() {
        assert_eq!(status, "typed_Residual_or_explicit_unsupported_if_parsed");
    }

    let schema = RelationSchema::new(
        "s",
        [("id", "VARCHAR"), ("k", "INTEGER"), ("x", "INTEGER")]
            .into_iter()
            .map(|(column, datatype)| {
                SchemaColumn::from_sql_type(column, datatype, "postgresql").expect("source type")
            })
            .collect(),
    )
    .expect("typed source");
    let catalog = RelationCatalog::from_schemas(&[schema]).expect("source catalog");
    let source = PhysicalWriteState::new(
        "s",
        source_rows
            .iter()
            .map(|row| {
                PhysicalWriteRow::new(
                    row.as_object()
                        .expect("source row")
                        .iter()
                        .map(|(key, value)| {
                            let scalar = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(n) = value.as_i64() {
                                LiteralValue::Number(n.to_string())
                            } else {
                                LiteralValue::Text(
                                    value.as_str().expect("row identity").to_string(),
                                )
                            };
                            (key.clone(), scalar)
                        })
                        .collect(),
                )
                .expect("typed source row")
            })
            .collect(),
    )
    .expect("complete source");

    for dialect in expected_dialects {
        let expected = &parser_cells[dialect];
        assert_eq!(expected["owner"], "TASK-73");
        assert_eq!(expected["engine"], "not_certified");
        assert_eq!(expected["parser"], "may_parse_or_reject");
        assert_eq!(
            expected["canonical"],
            "Residual_or_explicit_unsupported_if_parsed"
        );
        let parser = dialect_from_name(dialect).expect("project dialect");
        if Parser::parse_sql(parser.as_ref(), sql).is_err() {
            continue;
        }
        let input = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            boundary["id"].as_str().expect("boundary ID"),
            &input,
            dialect,
            parser.as_ref(),
        )];
        match analyze_configured_inputs_with_catalog(&configured, &catalog) {
            Err(error) => assert!(!error.to_string().trim().is_empty()),
            Ok(bundle) => {
                let target = bundle.layers().last().expect("analyzed target").id();
                let proof = physical_finite_aggregate_bag_plan(
                    &bundle,
                    target,
                    std::slice::from_ref(&source),
                );
                assert!(
                    matches!(proof, PhysicalFiniteAggregateProof::Residual { .. }),
                    "{dialect}: unproven GROUPING semantics promoted to Exact: {proof:?}"
                );
            }
        }
    }
}

#[test]
fn parent_multijoin_projection_acceptance_pins_independent_existing_oracles() {
    let parent = freeze(include_str!("../docs/task-73-acceptance-freeze.json"));
    let child = freeze(include_str!("../docs/task-115-acceptance-freeze.json"));
    let evidence = &parent["upstream_composition"];
    assert_eq!(evidence["owner"], "TASK-115");
    assert_eq!(evidence["source_of_truth"], "docs/task-115-acceptance-freeze.json");
    assert_eq!(evidence["mandatory_case_ids"], json!(["71-C115-P-GROUP", "71-C115-N-GROUP"]));
    assert_eq!(evidence["mandatory_dialect_cells"], 26);
    assert_eq!(evidence["mandatory_adapter_cells"], 6);
    assert_eq!(evidence["native_engine"], "duckdb");
    assert_eq!(evidence["native_vendor_engines_certified"], false);
    assert_eq!(evidence["parent_oracle"], "tests/task_73_composed_parent.rs");
    assert_eq!(evidence["existing_canonical_oracle"], "tests/task_115_finite_chain_bags.rs");
    assert_eq!(evidence["existing_duckdb_oracle"], "tests/task_115_duckdb_matrix.rs");
    assert_eq!(evidence["existing_adapter_oracle"], "tests/task_115_finite_adapter_parity.rs");
    assert_eq!(parent["portable_gate"]["mandatory_upstream_composition_cases"], 2);
    assert_eq!(
        parent["portable_gate"]["mandatory_upstream_composition_dialect_cells"],
        26
    );
    assert_eq!(
        parent["portable_gate"]["mandatory_upstream_composition_adapter_cells"],
        6
    );
    for id in ["71-C115-P-GROUP", "71-C115-N-GROUP"] {
        let case = rows(&child, "cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("independently frozen, required three-source case");
        assert_eq!(case["variant"], "downstream_group_count");
        assert_eq!(case["acceptance_status"], "verified_exact_canonical_and_independent_duckdb_bags");
        assert_eq!(case["dialect_matrix"].as_array().expect("dialect law").len(), 13);
        assert_eq!(case["adapter_matrix"].as_array().expect("adapter law").len(), 3);
        assert!(case["expected_terminal_bag"].is_array());
        assert!(case["selected_physical_triplets"].is_array());
        assert!(case["rejected_not_true_triplets"].is_array());
        assert!(case["terminal_domains"]["k"].is_object());
        assert!(case["terminal_domains"]["n"].is_object());
        assert_eq!(case["physical_source_bags"].as_object().expect("source bag").len(), 3);
        for cell in case["dialect_matrix"].as_array().expect("frozen parser matrix") {
            assert_eq!(cell["canonical_status"], "verified_portable_canonical_source_bag");
        }
        for cell in case["adapter_matrix"].as_array().expect("frozen adapter matrix") {
            assert_eq!(
                cell["canonical_population_status"],
                "verified_with_explicit_physical_attestation"
            );
        }
    }
}
