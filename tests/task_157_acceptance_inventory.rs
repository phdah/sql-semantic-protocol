//! TASK-157 task-owned executable acceptance inventory.
//! This checks that frozen exact outcomes are complete, bounded and
//! independently associated with their original child/parent gates.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-157-acceptance-freeze.json")).unwrap()
}

#[test]
fn every_frozen_ddl_cell_has_one_typed_complete_target_bag_and_truth_partition() {
    let f = frozen();
    assert_eq!(f["version"], 2);
    assert_eq!(
        f["previous_snapshot"],
        "docs/task-157-acceptance-freeze-v1.json"
    );
    assert_eq!(f["dialect_expected"].as_object().unwrap().len(), 13);
    assert_eq!(f["adapter_expected"].as_object().unwrap().len(), 9);
    assert_eq!(f["owner"], "TASK-157");
    assert_eq!(f["parent"], "TASK-83");
    assert_eq!(
        f["fixture_ids"],
        serde_json::json!(["83-C157-P", "83-C157-N"])
    );
    assert_eq!(f["task_ac"], serde_json::json!([1, 2, 3, 4, 5]));
    assert_eq!(f["original_parent_ac"], serde_json::json!([1, 3, 4, 5, 6]));
    assert_eq!(f["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(f["evidence_sources"].as_array().unwrap().len(), 3);
    assert_eq!(f["coverage"]["dialect_cells_required"], 117);
    assert_eq!(f["coverage"]["adapter_cells_required"], 27);
    assert_eq!(f["coverage"]["oracle_required"], 9);
    let cases = f["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 9);

    let mut ids = BTreeSet::new();
    let mut classes = BTreeMap::<String, usize>::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate fixture {id}");
        assert!(id.starts_with("83-C157-P-") || id.starts_with("83-C157-N-"));
        assert!(!case["variant"].as_str().unwrap().is_empty());
        assert!(!case["sql"].as_array().unwrap().is_empty());
        assert!(!case["target"].as_str().unwrap().is_empty());
        let class = case["classification"].as_str().unwrap();
        *classes.entry(class.to_string()).or_default() += 1;
        let raw = case["initial"]["raw"].as_array().unwrap();
        let accepted = case["source_truth"]["true_ordinals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        let rejected = case["source_truth"]["not_true_ordinals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        if class != "Residual" {
            let allowed = if id.contains("logical-view") {
                raw.len() + 1
            } else {
                raw.len()
            };
            let union = accepted
                .iter()
                .chain(&rejected)
                .copied()
                .collect::<BTreeSet<_>>();
            assert_eq!(
                union,
                (0..allowed).collect(),
                "{id}: every source occurrence accounted for"
            );
            assert_eq!(
                accepted.len() + rejected.len(),
                allowed,
                "{id}: no duplicate row identity"
            );
        }
        if class == "Exact" {
            assert_eq!(case["requirement"], "exact_required");
            let values = case["after"].as_array().unwrap();
            let domain = &case["domain"];
            assert_eq!(domain["type"], "INTEGER");
            let actual = values
                .iter()
                .filter_map(|row| row["k"].as_i64())
                .collect::<Vec<_>>();
            assert_eq!(
                actual.iter().min().copied(),
                domain["lower"].as_i64(),
                "{id}"
            );
            assert_eq!(
                actual.iter().max().copied(),
                domain["upper"].as_i64(),
                "{id}"
            );
            assert_eq!(domain["lower_inclusive"], !actual.is_empty(), "{id}");
            assert_eq!(domain["upper_inclusive"], !actual.is_empty(), "{id}");
            assert_eq!(
                domain["nullable"],
                values.iter().any(|row| row["k"].is_null()),
                "{id}"
            );
            let mut frequencies = BTreeMap::new();
            for row in values {
                let key = row["k"]
                    .as_i64()
                    .map_or("NULL".to_owned(), |n| n.to_string());
                *frequencies.entry(key).or_insert(0_u64) += 1;
            }
            let expected = domain["frequencies"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.clone(), value.as_u64().unwrap()))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(frequencies, expected, "{id}: complete value frequency bag");
        } else if class == "Impossible" {
            assert!(case["after"].is_null());
            assert!(!case["impossible_reason"].as_str().unwrap().is_empty());
        } else if class == "Residual" {
            assert_eq!(case["requirement"], "conditional");
            assert_eq!(case["owner"], "TASK-157");
            assert!(!case["release_disposition"].as_str().unwrap().is_empty());
        } else {
            panic!("unknown proof strength {class}");
        }
    }
    assert_eq!(classes.get("Exact"), Some(&7));
    assert_eq!(classes.get("Impossible"), Some(&1));
    assert_eq!(classes.get("Residual"), Some(&1));
    for index in 1..=3 {
        let key = index.to_string();
        let linked = f["acceptance_mapping"][&key].as_array().unwrap();
        assert_eq!(linked.len(), cases.len());
        assert_eq!(
            linked
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            ids,
            "AC #{index} must map every mandatory/conditional case"
        );
    }
    assert_eq!(f["acceptance_mapping"]["4"].as_array().unwrap().len(), 4);
    assert_eq!(f["acceptance_mapping"]["5"].as_array().unwrap().len(), 8);
}

#[test]
fn coverage_manifest_points_to_same_immutable_version_without_parent_signoff() {
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let entry = &manifest["task_157_ddl_identity_acceptance"];
    assert_eq!(entry["freeze"], "docs/task-157-acceptance-freeze.json");
    assert_eq!(entry["freeze_version"], 2);
    assert_eq!(entry["mandatory_cases"], 8);
    assert_eq!(entry["conditional_cases"], 1);
    assert_eq!(entry["parent_signed_off"], false);
}
