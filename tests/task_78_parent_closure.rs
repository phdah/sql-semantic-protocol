//! Independent TASK-78 parent closure inventory over merged CTE child contracts.
//!
//! Child tests own independently executed DuckDB and direct/dbt/ODCS proof
//! oracles. This guard locks their typed required/conditional inventory and
//! original acceptance mapping without inventing vendor or materialization laws.

use serde_json::{json, Value};
use std::collections::BTreeSet;

const CHILDREN: [(&str, &str); 4] = [
    (
        "TASK-141",
        include_str!("../docs/task-141-proof-matrix.json"),
    ),
    (
        "TASK-142",
        include_str!("../docs/task-142-acceptance-freeze.json"),
    ),
    (
        "TASK-143",
        include_str!("../docs/task-143-acceptance-freeze.json"),
    ),
    (
        "TASK-144",
        include_str!("../docs/task-144-acceptance-freeze.json"),
    ),
];

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw).expect("versioned acceptance contract")
}

fn strings(value: &Value) -> BTreeSet<String> {
    let items = value.as_array().expect("stable inventory");
    let unique: BTreeSet<String> = items
        .iter()
        .map(|item| item.as_str().expect("string identifier").to_owned())
        .collect();
    assert_eq!(unique.len(), items.len(), "duplicate inventory identifier");
    unique
}

fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|s| !s.trim().is_empty())
}

fn verify_mandatory_outcome(child: &Value, task: &str, id: &str) {
    assert!(
        child["expected_terminal_bag"].is_array()
            || child["terminal_bag"].is_array()
            || child["expected_terminal"].is_array(),
        "{task} {id}: no frozen complete terminal bag"
    );
    assert!(
        child["expected_terminal_domains"].is_object()
            || child["terminal_domain"].is_object()
            || child["terminal_domains"].is_object()
            || child["domains"].is_object(),
        "{task} {id}: no typed terminal domain"
    );
    if task == "TASK-143" || task == "TASK-144" {
        assert!(nonempty(&child["true_proof"]), "{id}: missing TRUE proof");
        assert!(
            nonempty(&child["not_true_proof"]),
            "{id}: missing NOT TRUE proof"
        );
    } else if task == "TASK-141" {
        assert_eq!(
            child["expected_proof"], "Exact",
            "{id}: unexpectedly nonexact"
        );
    } else {
        assert!(
            child["positive"].is_object() || child["proof"] == "Impossible",
            "{id}: no constructive or impossible proof"
        );
    }
}

#[test]
fn every_child_case_has_an_independent_parent_inventory_entry() {
    let parent = parse(include_str!(
        "../docs/task-78-parent-acceptance-freeze.json"
    ));
    assert_eq!(parent["task"], "TASK-78");
    assert_eq!(parent["version"], 1);
    assert_eq!(parent["release_approved"], false);
    assert_eq!(parent["missing_mandatory_child_cases"], json!([]));
    let mandatory = strings(&parent["mandatory_case_ids"]);
    let conditional = strings(&parent["conditional_case_ids"]);
    assert_eq!(mandatory.len(), 20);
    assert_eq!(conditional.len(), 9);
    assert!(mandatory.is_disjoint(&conditional));
    assert_eq!(
        parent["case_index"].as_object().expect("case index").len(),
        29
    );
    let dialects = strings(&parent["dialect_identities"]);
    assert_eq!(dialects.len(), 14); // 13 baseline dialects and the postgres alias.
    let adapters = strings(&parent["canonical_evidence_sources"]);
    assert_eq!(adapters.len(), 3);

    let mut seen_mandatory = BTreeSet::new();
    let mut seen_conditional = BTreeSet::new();
    let mut seen_fixtures = BTreeSet::new();
    for (task, raw) in CHILDREN {
        let child = parse(raw);
        let record = &parent["merged_children"][task];
        assert_eq!(child["owner"], task);
        assert_eq!(child["parent"], "TASK-78");
        assert_eq!(record["version"], child["version"]);
        assert!(nonempty(&record["merged_pr"]));
        let fixtures = strings(&record["fixture_ids"]);
        assert_eq!(fixtures.len(), 2);
        seen_fixtures.extend(fixtures);
        let declared_mandatory = strings(&record["mandatory_case_ids"]);
        let declared_conditional = strings(&record["conditional_case_ids"]);
        let mut actual_mandatory = BTreeSet::new();
        let mut actual_conditional = BTreeSet::new();

        let items: Vec<(&Value, bool)> = if task == "TASK-141" {
            child["mandatory"]
                .as_array()
                .expect("TASK-141 mandatory")
                .iter()
                .map(|case| (case, false))
                .chain(
                    child["conditional"]
                        .as_array()
                        .expect("TASK-141 conditional")
                        .iter()
                        .map(|case| (case, true)),
                )
                .collect()
        } else {
            child["cases"]
                .as_array()
                .expect("child cases")
                .iter()
                .map(|case| {
                    let classification = case["classification"].as_str().expect("typed case");
                    (case, classification.starts_with("conditional"))
                })
                .collect()
        };
        for (case, is_conditional) in items {
            let id = case["id"].as_str().expect("case id");
            let index = &parent["case_index"][id];
            assert_eq!(index["owner"], task);
            assert_eq!(
                index["classification"],
                if is_conditional {
                    "conditional"
                } else {
                    "mandatory"
                }
            );
            let fixture_id = index["fixture_id"].as_str().expect("fixture");
            assert!(record["fixture_ids"]
                .as_array()
                .expect("fixtures")
                .iter()
                .any(|fixture| fixture.as_str() == Some(fixture_id)));
            if is_conditional {
                assert_eq!(case["owner"], task, "{id}: missing boundary owner");
                assert!(
                    nonempty(&case["release_disposition"]),
                    "{id}: missing reviewed release disposition"
                );
                let proof = case
                    .get("expected_proof")
                    .or_else(|| case.get("expected_physical"))
                    .or_else(|| case.get("proof"))
                    .unwrap_or(&case["true_proof"]);
                assert!(
                    proof.as_str().is_some_and(|s| s.contains("Residual")),
                    "{id}: conditional case must fail closed"
                );
                assert!(actual_conditional.insert(id.to_owned()), "{id}: duplicate");
                assert!(seen_conditional.insert(id.to_owned()), "{id}: duplicate");
            } else {
                verify_mandatory_outcome(case, task, id);
                assert!(actual_mandatory.insert(id.to_owned()), "{id}: duplicate");
                assert!(seen_mandatory.insert(id.to_owned()), "{id}: duplicate");
            }
        }
        assert_eq!(
            actual_mandatory, declared_mandatory,
            "{task}: missing required case"
        );
        assert_eq!(
            actual_conditional, declared_conditional,
            "{task}: missing boundary"
        );
        if task == "TASK-141" {
            for case in child["mandatory"].as_array().expect("mandatory") {
                let cells = case["dialect_cells"].as_array().expect("dialect cells");
                let identities: BTreeSet<String> = cells
                    .iter()
                    .map(|cell| cell["dialect"].as_str().expect("dialect").to_owned())
                    .collect();
                assert_eq!(identities, dialects);
            }
        } else {
            let cells = child["dialects"].as_array().expect("dialect cells");
            let identities: BTreeSet<String> = cells
                .iter()
                .map(|cell| cell["name"].as_str().expect("dialect identity").to_owned())
                .collect();
            assert_eq!(identities.len(), cells.len(), "{task}: duplicate dialect");
            assert_eq!(identities, dialects, "{task}: dialect coverage");
        }
    }
    assert_eq!(seen_mandatory, mandatory);
    assert_eq!(seen_conditional, conditional);
    assert_eq!(seen_fixtures, strings(&parent["fixture_ids"]));
}

#[test]
fn original_acceptance_and_manifest_do_not_confuse_parent_with_release() {
    let freeze = parse(include_str!(
        "../docs/task-78-parent-acceptance-freeze.json"
    ));
    let manifest = parse(include_str!("../docs/coverage-manifest.json"));
    let criteria = freeze["original_acceptance_criteria"]
        .as_array()
        .expect("original parent requirements");
    assert_eq!(criteria.len(), 6);
    let mandatory = strings(&freeze["mandatory_case_ids"]);
    let conditional = strings(&freeze["conditional_case_ids"]);
    for (i, criterion) in criteria.iter().enumerate() {
        assert_eq!(criterion["id"], i + 1);
        assert!(nonempty(&criterion["obligation"]));
        let required = strings(&criterion["mandatory_case_ids"]);
        assert!(!required.is_empty(), "AC {} has no mandatory oracle", i + 1);
        assert!(
            required.is_subset(&mandatory),
            "AC {} has unknown proof",
            i + 1
        );
        assert!(
            strings(&criterion["conditional_case_ids"]).is_subset(&conditional),
            "AC {} has unknown residual",
            i + 1
        );
    }
    let entry = &manifest["task_78_parent_acceptance"];
    assert_eq!(entry["owner"], "TASK-78");
    assert_eq!(
        entry["freeze"],
        "docs/task-78-parent-acceptance-freeze.json"
    );
    assert_eq!(entry["original_ac"], json!([1, 2, 3, 4, 5, 6]));
    assert_eq!(
        entry["mandatory_cases"].as_u64(),
        Some(mandatory.len() as u64)
    );
    assert_eq!(
        entry["conditional_cases"].as_u64(),
        Some(conditional.len() as u64)
    );
    assert_eq!(entry["parent_signed_off"], freeze["parent_signed_off"]);
    assert_eq!(entry["release_approved"], false);
}
