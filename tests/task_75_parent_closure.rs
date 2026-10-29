//! TASK-75 original acceptance: independent closure over merged child contracts.
//!
//! The concrete physical and native DuckDB oracles remain in TASK-129..132 tests.
//! This public integration guard prevents omitted mandatory variants, lost adapter
//! or parser cells, and accidental promotion of reviewed conditional boundaries.

use serde_json::{json, Value};
use std::collections::BTreeSet;

const CHILDREN: [(&str, &str); 4] = [
    (
        "TASK-129",
        include_str!("../docs/task-129-acceptance-freeze.json"),
    ),
    (
        "TASK-130",
        include_str!("../docs/task-130-acceptance-freeze.json"),
    ),
    (
        "TASK-131",
        include_str!("../docs/task-131-acceptance-freeze.json"),
    ),
    (
        "TASK-132",
        include_str!("../docs/task-132-acceptance-freeze.json"),
    ),
];

fn load(text: &str) -> Value {
    serde_json::from_str(text).expect("valid versioned acceptance contract")
}

fn strings(value: &Value) -> BTreeSet<String> {
    let values = value.as_array().expect("frozen array");
    let unique: BTreeSet<String> = values
        .iter()
        .map(|value| value.as_str().expect("stable string").to_owned())
        .collect();
    assert_eq!(unique.len(), values.len(), "duplicate frozen IDs");
    unique
}

fn aliases(name: &str) -> &str {
    match name {
        "postgres" => "postgresql",
        "sql" => "direct_sql",
        "dbt" => "dbt_compiled",
        "odcs" => "odcs_schema",
        other => other,
    }
}

fn keys(value: &Value) -> BTreeSet<String> {
    let cells = value.as_object().expect("explicit coverage cells");
    let normalized: BTreeSet<String> = cells.keys().map(|name| aliases(name).to_owned()).collect();
    assert_eq!(
        normalized.len(),
        cells.len(),
        "duplicate canonical dialect/adapter cell after alias normalization"
    );
    normalized
}

fn exact_statuses(value: &Value, expected: &[&str], id: &str, matrix: &str) {
    for (name, status) in value.as_object().expect("frozen coverage matrix") {
        let status = status.as_str().expect("typed frozen coverage status");
        assert!(
            expected.contains(&status),
            "{id}: {matrix} {name} is not an approved verified/conditional status: {status}"
        );
    }
}

fn physical_source_bags(case: &Value, id: &str) {
    let sources = case
        .get("source_bags")
        .or_else(|| case.get("raw_seed_bags"))
        .expect("physical source bags");
    let sources = sources.as_object().expect("physical relation inventory");
    assert!(!sources.is_empty(), "{id}: missing physical input relations");
    for (relation, rows) in sources {
        assert!(rows.is_array(), "{id}: {relation} must be a complete row bag");
    }
}

fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

#[test]
fn every_original_criterion_cites_frozen_verified_child_cases() {
    let parent = load(include_str!("../docs/task-75-acceptance-freeze.json"));
    assert_eq!(parent["task"], "TASK-75");
    assert_eq!(parent["version"], 1);
    assert_eq!(parent["parent_signed_off"], true);
    assert_eq!(parent["release_approved"], false);
    assert!(parent["mandatory_unverified"]
        .as_array()
        .is_some_and(Vec::is_empty));

    let expected_mandatory = strings(&parent["mandatory_case_ids"]);
    let expected_conditional = strings(&parent["conditional_case_ids"]);
    assert_eq!(expected_mandatory.len(), 40);
    assert_eq!(expected_conditional.len(), 10);
    assert!(expected_mandatory.is_disjoint(&expected_conditional));

    let dialects = strings(&parent["portable_gate"]["required_dialects"]);
    let adapters = strings(&parent["portable_gate"]["required_adapters"]);
    assert_eq!(dialects.len(), 13);
    assert_eq!(
        adapters,
        strings(&json!(["direct_sql", "dbt_compiled", "odcs_schema"]))
    );

    assert_eq!(
        keys(&parent["source_of_truth"]),
        strings(&json!(["TASK-129", "TASK-130", "TASK-131", "TASK-132"]))
    );

    let mut mandatory = BTreeSet::new();
    let mut conditional = BTreeSet::new();
    let mut fixtures = BTreeSet::new();
    let mut cardinality_errors = 0;

    for (task, raw) in CHILDREN {
        let child = load(raw);
        assert_eq!(child["owner"], task);
        assert_eq!(child["parent"], "TASK-75");
        let record = &parent["source_of_truth"][task];
        assert_eq!(record["version"], child["version"]);
        assert_eq!(
            record["freeze"],
            format!("docs/task-{}-acceptance-freeze.json", &task[5..])
        );
        let actual_fixtures = strings(&child["fixture_ids"]);
        assert_eq!(strings(&record["fixture_ids"]), actual_fixtures);
        fixtures.extend(actual_fixtures);

        let mut child_mandatory = BTreeSet::new();
        let mut child_conditional = BTreeSet::new();

        for case in child["cases"].as_array().expect("child case inventory") {
            let id = case["id"].as_str().expect("stable case ID");
            assert!(nonempty(&case["sql"]), "{id}: no SQL or fixture");
            physical_source_bags(case, id);
            if task != "TASK-131" {
                assert_eq!(
                    case.get("fixture").or_else(|| case.get("fixture_id")),
                    case.get("fixture").or_else(|| case.get("fixture_id"))
                        .filter(|fixture| actual_fixtures.contains(fixture.as_str().expect("fixture ID"))),
                    "{id}: missing or unowned fixture ID"
                );
            }

            let parser = case
                .get("parser_status")
                .or_else(|| case.get("parser_status_by_dialect"))
                .expect("frozen dialect matrix");
            let adapter = case
                .get("adapter_status")
                .or_else(|| case.get("expected_adapter_cells"))
                .expect("frozen adapter matrix");
            assert_eq!(keys(parser), dialects, "{id}: parser coverage missing");
            assert_eq!(keys(adapter), adapters, "{id}: adapter coverage missing");
            let (parser_statuses, adapter_statuses): (&[&str], &[&str]) = match task {
                "TASK-129" => (&["parser_equivalent"], &["verified"]),
                "TASK-130" => (&["verified_parser_and_canonical"], &["verified"]),
                "TASK-131" if case["expected_proof"] == "Residual" => {
                    (&["parsed"], &["Residual_conditional"])
                }
                "TASK-131" => (&["parsed"], &["Exact_or_CardinalityViolation_required"]),
                "TASK-132" => (
                    &[
                        "verified_sqlparser_canonical_complete_source_bag_no_native_engine_claim",
                        "verified_compiled_sql_shared_parser_syntax_four_producers_no_native_engine_claim",
                    ],
                    &[
                        "verified_equivalent_canonical_full_source_bag",
                        "verified_exact_full_canonical_source_to_terminal_producer_chain",
                    ],
                ),
                _ => panic!("unexpected child {task}"),
            };
            exact_statuses(parser, parser_statuses, id, "parser");
            exact_statuses(adapter, adapter_statuses, id, "adapter");

            if task == "TASK-131" && case["expected_proof"] == "Residual" {
                assert_eq!(
                    case["terminal_oracle"]["status"],
                    "conditional_native_engine_only"
                );
                assert_eq!(case["expect"]["owner"], task, "{id}: missing conditional owner");
                assert_eq!(case["expect"]["type"], "conditional", "{id}");
                assert!(nonempty(&case["expect"]["reason"]), "{id}: no conditional reason");
                assert!(
                    nonempty(&case["release_disposition"]),
                    "{id}: missing reviewed release disposition"
                );
                assert!(child_conditional.insert(id.to_owned()), "{id}");
                assert!(conditional.insert(id.to_owned()), "{id}");
                continue;
            }

            if task == "TASK-131" {
                let proof = case["expected_proof"].as_str().expect("typed proof");
                assert!(matches!(proof, "Exact" | "CardinalityViolation"), "{id}");
                assert!(case["terminal_oracle"].is_object(), "{id}");
                assert!(case["truth_partition"].is_object(), "{id}");
                if proof == "CardinalityViolation" {
                    cardinality_errors += 1;
                    assert_eq!(
                        case["terminal_oracle"]["status"],
                        "runtime_cardinality_error"
                    );
                } else {
                    assert_eq!(case["terminal_oracle"]["status"], "exact");
                    assert!(case["terminal_oracle"]["rows"].is_array(), "{id}");
                }
            } else {
                if task == "TASK-129" {
                    assert_eq!(case["duckdb_source_and_terminal_oracle"], "verified", "{id}");
                } else if task == "TASK-130" {
                    assert!(
                        matches!(
                            case["duckdb_source_and_terminal_oracle"].as_str(),
                            Some("verified_native_scalar_sql")
                                | Some("verified_independent_fieldwise_sql_reference")
                        ),
                        "{id}: no verified native execution reference"
                    );
                } else if task == "TASK-132" {
                    assert_eq!(case["status"], "verified_mandatory_test_evidence", "{id}");
                }
                assert!(nonempty(&case["positive_proof"]), "{id}");
                assert!(nonempty(&case["not_true_proof"]), "{id}");
                assert!(
                    case["expected_terminal_bag"].is_array() || case["terminal"].is_array(),
                    "{id}: no complete terminal bag"
                );
                assert!(
                    case["terminal_value_domain"].is_object()
                        || case["terminal_domain"].is_object(),
                    "{id}: no typed terminal domain"
                );
                if task == "TASK-132" {
                    assert_eq!(case["required"], true, "{id}");
                } else {
                    assert!(case["selected_outer_ids"].is_array(), "{id}");
                    assert!(case["rejected_not_true_outer_ids"].is_array(), "{id}");
                }
            }
            assert!(child_mandatory.insert(id.to_owned()), "{id}");
            assert!(mandatory.insert(id.to_owned()), "{id}");
        }

        if task != "TASK-131" {
            for boundary in child["conditional"]
                .as_array()
                .expect("conditional inventory")
            {
                let id = boundary["id"].as_str().expect("conditional ID");
                assert_eq!(boundary["owner"], task);
                assert!(nonempty(&boundary["reason"]), "{id}");
                assert!(
                    nonempty(&boundary["disposition"])
                        || nonempty(&boundary["reviewed_release_disposition"]),
                    "{id}: missing reviewed disposition"
                );
                assert_eq!(
                    boundary["classification"].as_str().unwrap_or("Residual"),
                    "Residual",
                    "{id}: unreviewed native exactness promotion"
                );
                assert!(child_conditional.insert(id.to_owned()), "{id}");
                assert!(conditional.insert(id.to_owned()), "{id}");
            }
        }
        assert_eq!(strings(&record["mandatory_case_ids"]), child_mandatory);
        assert_eq!(strings(&record["conditional_case_ids"]), child_conditional);
    }

    assert_eq!(mandatory, expected_mandatory);
    assert_eq!(conditional, expected_conditional);
    assert_eq!(cardinality_errors, 1);
    assert_eq!(fixtures, strings(&parent["fixture_ids"]));
    assert_eq!(parent["portable_gate"]["mandatory_cases"], 40);
    assert_eq!(parent["portable_gate"]["conditional_cases"], 10);
    assert_eq!(parent["portable_gate"]["mandatory_parser_cells"], 520);
    assert_eq!(parent["portable_gate"]["mandatory_adapter_cells"], 120);
    assert_eq!(
        parent["portable_gate"]["mandatory_parser_cells"].as_u64(),
        Some((mandatory.len() * dialects.len()) as u64)
    );
    assert_eq!(
        parent["portable_gate"]["mandatory_adapter_cells"].as_u64(),
        Some((mandatory.len() * adapters.len()) as u64)
    );

    let criteria = parent["original_acceptance_criteria"]
        .as_array()
        .expect("all six original AC");
    assert_eq!(criteria.len(), 6);
    for (index, criterion) in criteria.iter().enumerate() {
        assert_eq!(criterion["id"], index + 1);
        assert!(nonempty(&criterion["obligation"]));
        let required = strings(&criterion["mandatory_case_ids"]);
        assert!(
            !required.is_empty(),
            "AC {} has no mandatory proof",
            index + 1
        );
        assert!(required.is_subset(&mandatory));
        assert!(strings(&criterion["conditional_case_ids"]).is_subset(&conditional));
    }
}

#[test]
fn coverage_registry_preserves_parent_signoff_separately_from_release() {
    let freeze = load(include_str!("../docs/task-75-acceptance-freeze.json"));
    let manifest = load(include_str!("../docs/coverage-manifest.json"));
    let row = &manifest["task_75_parent_acceptance"];
    assert_eq!(row["owner"], "TASK-75");
    assert_eq!(row["freeze"], "docs/task-75-acceptance-freeze.json");
    assert_eq!(row["original_ac"], json!([1, 2, 3, 4, 5, 6]));
    assert_eq!(
        row["mandatory_cases"],
        freeze["portable_gate"]["mandatory_cases"]
    );
    assert_eq!(
        row["conditional_cases"],
        freeze["portable_gate"]["conditional_cases"]
    );
    assert_eq!(row["mandatory_parser_cells"], 520);
    assert_eq!(row["mandatory_adapter_cells"], 120);
    assert_eq!(
        strings(&row["merged_children"]),
        strings(&json!(["TASK-129", "TASK-130", "TASK-131", "TASK-132"]))
    );
    assert_eq!(row["parent_regression"], "tests/task_75_parent_closure.rs");
    assert_eq!(row["parent_signed_off"], true);
    assert_eq!(row["release_approved"], false);
}
