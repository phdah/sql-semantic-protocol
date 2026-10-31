//! Independent TASK-81 parent acceptance boundary over four merged write children.
//! The finite crosswalk is checked against child-owned physical and native oracles.
//! Parser acceptance must never be mistaken for native vendor execution.

use serde_json::{json, Value};
use std::collections::BTreeSet;

const CHILDREN: [(&str, &str); 4] = [
    ("TASK-149", include_str!("../docs/task-149-acceptance-freeze.json")),
    ("TASK-150", include_str!("../docs/task-150-acceptance-freeze.json")),
    ("TASK-151", include_str!("../docs/task-151-acceptance-freeze.json")),
    ("TASK-152", include_str!("../docs/task-152-acceptance-freeze.json")),
];

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("valid frozen contract")
}

fn names(value: &Value) -> BTreeSet<String> {
    let values = value.as_array().expect("inventory array");
    let found = values
        .iter()
        .map(|item| item.as_str().expect("stable identifier").to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(found.len(), values.len(), "duplicate inventory identifier");
    found
}

fn canonical_proof(case: &Value) -> &'static str {
    let raw = case["proof"].as_str().or_else(|| case["classification"].as_str()).expect("proof classification");
    match raw.to_ascii_lowercase().as_str() {
        "exact" | "exact_with_attested_default" | "exact_not_true" | "mandatory_exact" => "Exact",
        "impossible" | "mandatory_impossible" => "Impossible",
        "residual" | "residual_missing_attestation" | "mandatory_residual" | "conditional_residual" => "Residual",
        other => panic!("unknown frozen proof: {other}"),
    }
}

fn exact_target_bag(case: &Value) -> bool {
    case["target_after"].is_array() || case["expected_final"].is_array()
}

fn typed_target_domains(case: &Value) -> bool {
    ["typed_domains", "domain_contract", "expected_final_domains"]
        .iter()
        .any(|name| case[*name].is_object())
}

#[test]
fn inventory_is_complete_and_exactness_does_not_hide_residuals() {
    let parent = parse(include_str!("../docs/task-81-parent-acceptance-freeze.json"));
    assert_eq!(parent["task"], "TASK-81");
    assert_eq!(parent["version"], 1);
    assert_eq!(parent["parent_signed_off"], false);
    assert_eq!(parent["release_approved"], false);

    let mandatory = names(&parent["mandatory_case_ids"]);
    let conditional = names(&parent["conditional_case_ids"]);
    assert_eq!(mandatory.len(), 34);
    assert_eq!(conditional.len(), 12);
    assert!(mandatory.is_disjoint(&conditional));
    assert_eq!(parent["case_index"].as_object().expect("case index").len(), 46);

    let mut seen = BTreeSet::new();
    let mut seen_fixtures = BTreeSet::new();
    let mut exact = 0;
    let mut impossible = 0;
    let mut residual = 0;

    for (owner, raw) in CHILDREN {
        let child = parse(raw);
        let record = &parent["merged_children"][owner];
        assert_eq!(child["owner"], owner);
        assert_eq!(child["parent"], "TASK-81");
        assert_eq!(record["version"], child["version"]);
        assert!(record["merged_pr"].as_str().is_some_and(|s| s.starts_with("https://github.com/")));
        let child_fixtures = names(&child["fixture_ids"]);
        assert_eq!(child_fixtures, names(&record["fixture_ids"]));
        assert_eq!(child_fixtures.len(), 2);
        seen_fixtures.extend(child_fixtures);

        let dialects = names(&child["dialects"]);
        assert_eq!(dialects, names(&parent["dialect_identities"]));
        assert_eq!(dialects.len(), 13);
        assert_eq!(names(&child["adapters"]), names(&parent["adapter_sources"]));

        for case in child["cases"].as_array().expect("frozen cases") {
            let id = case["id"].as_str().expect("case ID");
            assert!(seen.insert(id.to_owned()), "duplicate case: {id}");
            let index = &parent["case_index"][id];
            assert_eq!(index["owner"], owner, "{id}");
            assert_eq!(index["evidence"], "case", "{id}");
            assert_eq!(index["fixture_id"], case["fixture_id"], "{id}");
            let classification = index["classification"].as_str().expect("classification");
            assert!(
                (classification == "mandatory" && mandatory.contains(id))
                    || (classification == "conditional" && conditional.contains(id)),
                "{id}: index classification drift"
            );
            let proof = canonical_proof(case);
            assert_eq!(index["proof"], proof, "{id}");
            match proof {
                "Exact" => {
                    exact += 1;
                    assert!(exact_target_bag(case), "{id}: no complete target after-bag");
                    assert!(typed_target_domains(case), "{id}: no typed domains");
                    if owner == "TASK-152" {
                        assert!(case["steps"].is_array(), "{id}: no ordered step evidence");
                    } else {
                        assert!(case["sql"].as_str().is_some(), "{id}: no SQL oracle");
                    }
                }
                "Impossible" => impossible += 1,
                "Residual" => {
                    residual += 1;
                    assert!(
                        classification == "conditional"
                            || id.starts_with("81-C149-N-")
                            || id.starts_with("81-C151-N-")
                            || id == "81-C152-N-missing-reference",
                        "{id}: residual is not an explicitly bounded negative case"
                    );
                }
                _ => unreachable!("canonical proof is closed"),
            }
        }

        if owner == "TASK-149" {
            for boundary in child["conditional"].as_array().expect("conditional boundaries") {
                let id = boundary["id"].as_str().expect("conditional ID");
                assert!(seen.insert(id.to_owned()), "duplicate conditional {id}");
                let index = &parent["case_index"][id];
                assert_eq!(index["owner"], owner);
                assert_eq!(index["evidence"], "root_conditional");
                assert_eq!(index["proof"], "Residual");
                assert_eq!(boundary["owner"], owner);
                assert!(boundary["disposition"].as_str().is_some_and(|s| !s.is_empty()));
                residual += 1;
            }
        }
        if owner == "TASK-152" {
            for boundary in child["conditional_boundaries"].as_array().expect("vendor boundaries") {
                let family = boundary["family"].as_str().expect("family");
                let id = format!("81-C152-C-{family}");
                assert!(seen.insert(id.clone()), "duplicate conditional {id}");
                let index = &parent["case_index"][&id];
                assert_eq!(index["owner"], owner);
                assert_eq!(index["proof"], "Residual");
                assert_eq!(index["evidence"], "root_conditional_boundary");
                assert_eq!(boundary["owner"], owner);
                assert_eq!(boundary["proof"], "Residual");
                assert!(boundary["release_disposition"].as_str().is_some_and(|s| !s.is_empty()));
                residual += 1;
            }
        }
    }
    assert_eq!(seen, mandatory.union(&conditional).cloned().collect());
    assert_eq!(seen_fixtures, names(&parent["fixture_ids"]));
    assert_eq!((exact, impossible, residual), (27, 3, 16));
}

#[test]
fn original_parent_criteria_and_manifest_are_independently_traceable() {
    let parent = parse(include_str!("../docs/task-81-parent-acceptance-freeze.json"));
    let manifest = parse(include_str!("../docs/coverage-manifest.json"));
    let mandatory = names(&parent["mandatory_case_ids"]);
    let conditional = names(&parent["conditional_case_ids"]);
    let acs = parent["original_acceptance_criteria"].as_array().expect("six parent criteria");
    assert_eq!(acs.len(), 6);
    let mut covered = BTreeSet::new();
    for (ordinal, ac) in acs.iter().enumerate() {
        assert_eq!(ac["id"], json!(ordinal + 1));
        assert!(ac["obligation"].as_str().is_some_and(|s| !s.is_empty()));
        let m = names(&ac["mandatory_case_ids"]);
        let c = names(&ac["conditional_case_ids"]);
        assert!(!m.is_empty(), "AC {} must have exact or explicit negative evidence", ordinal + 1);
        assert!(m.is_subset(&mandatory));
        assert!(c.is_subset(&conditional));
        covered.extend(m);
        covered.extend(c);
    }
    assert_eq!(covered, mandatory.union(&conditional).cloned().collect());
    let registry = &manifest["task_81_parent_acceptance"];
    assert_eq!(registry["owner"], "TASK-81");
    assert_eq!(registry["freeze"], "docs/task-81-parent-acceptance-freeze.json");
    assert_eq!(registry["original_ac"], json!([1, 2, 3, 4, 5, 6]));
    assert_eq!(registry["mandatory_cases"], 34);
    assert_eq!(registry["conditional_cases"], 12);
    assert_eq!(registry["parent_signed_off"], false);
    assert_eq!(registry["release_approved"], false);
}
