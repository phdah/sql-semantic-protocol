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

struct AcRequirement(u64, &'static [&'static str], &'static [&'static str], &'static [&'static str]);

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
const CONDITIONAL_GROUPINGS: &[&str] = &[
    "73-C124-N-3-4",
    "73-C124-N-3-5",
    "73-C124-N-3-6",
    "73-C124-N-3-7",
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
    for (criterion, AcRequirement(id, required, grouping, numeric)) in mappings.iter().zip(expected.iter()) {
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
    assert_eq!(parent["portable_gate"]["conditional_grouping_cases"], 4);
    assert_eq!(parent["portable_gate"]["conditional_parser_cells"], 52);
    assert_eq!(parent["portable_gate"]["conditional_adapter_cells"], 12);
    let groupings = rows(&parent, "grouping_boundaries");
    assert_eq!(groupings.len(), CONDITIONAL_GROUPINGS.len());
    for (boundary, expected_id) in groupings.iter().zip(CONDITIONAL_GROUPINGS) {
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
