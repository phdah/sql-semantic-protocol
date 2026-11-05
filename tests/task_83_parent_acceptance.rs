//! TASK-83 original AC and child-frozen-case accounting.
//! Independent native lifecycle oracles live in task_83_parent_lifecycle.rs.

use std::collections::BTreeSet;

use serde_json::Value;

#[test]
fn parent_registry_preserves_child_inventories_and_original_ac_ownership() {
    let freeze = frozen();
    assert_eq!(freeze["owner"], "TASK-83");
    assert_eq!(freeze["version"], 1);
    assert_eq!(freeze["independent_parent_signoff"], false);
    let paths = freeze["child_freezes"].as_array().unwrap();
    assert_eq!(paths.len(), 4);
    let children: [Value; 4] = [
        serde_json::from_str(include_str!("../docs/task-157-acceptance-freeze.json")).unwrap(),
        serde_json::from_str(include_str!("../docs/task-158-acceptance-freeze.json")).unwrap(),
        serde_json::from_str(include_str!("../docs/task-159-acceptance-freeze.json")).unwrap(),
        serde_json::from_str(include_str!("../docs/task-160-acceptance-freeze.json")).unwrap(),
    ];
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let records = [
        ("task_157_ddl_identity_acceptance", [7, 1, 1]),
        ("task_158_alter_column_transition_acceptance", [4, 2, 2]),
        ("task_159_ddl_lifecycle_acceptance", [6, 2, 10]),
        ("task_160_ordered_lifecycle_acceptance", [3, 2, 2]),
    ];
    let mut child_ids = BTreeSet::new();
    let mut case_ids = BTreeSet::new();
    for (index, child) in children.iter().enumerate() {
        let id = format!("TASK-{}", index + 157);
        assert_eq!(child["owner"], id);
        assert_eq!(child["parent"], "TASK-83");
        assert_eq!(
            paths[index],
            format!("docs/task-{}-acceptance-freeze.json", index + 157)
        );
        assert_eq!(child["dialects"].as_array().unwrap().len(), 13);
        child_ids.insert(id);
        let record = &manifest[records[index].0];
        assert_eq!(record["owner"], child["owner"]);
        assert_eq!(record["child_signed_off"], true);
        assert_eq!(record["mandatory_cells_verified"], true);
        assert!(
            record["ci_verified"] == true || record["implementation_ci_verified"] == true,
            "child {index} lacks a recorded verified implementation head"
        );
        let mut strengths = [0usize; 3];
        for case in child["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            assert!(id.starts_with("83-C"));
            assert!(case_ids.insert(id), "duplicate child fixture: {id}");
            let outcome = case["expected_outcome"]
                .as_str()
                .or_else(|| case["classification"].as_str())
                .unwrap();
            let rank = match outcome {
                "Exact" => 0,
                "Impossible" => 1,
                "Residual" => 2,
                other => panic!("{id}: unclassified child proof outcome {other}"),
            };
            strengths[rank] += 1;
        }
        assert_eq!(
            strengths, records[index].1,
            "child {index}: original exact/impossible/residual evidence changed"
        );
    }
    assert_eq!(case_ids.len(), 42, "preserve all 42 child-frozen cases");

    let mapping = freeze["original_parent_ac_mapping"].as_object().unwrap();
    assert_eq!(mapping.len(), 6);
    for ac in 1..=6 {
        let owners = mapping[&ac.to_string()].as_array().unwrap();
        assert!(!owners.is_empty());
        for owner in owners {
            let owner = owner.as_str().unwrap();
            assert!(child_ids.contains(owner));
            let child = children
                .iter()
                .find(|child| child["owner"] == owner)
                .unwrap();
            let original_ac = child["original_parent_ac"]
                .as_array()
                .or_else(|| child["original_parent_ac_mapping"].as_array())
                .unwrap();
            assert!(
                original_ac
                    .iter()
                    .any(|covered| covered.as_u64() == Some(ac)),
                "{owner} has no original parent AC {ac} evidence"
            );
        }
    }
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0]["requirement"], "mandatory");
    assert_eq!(cases[1]["requirement"], "mandatory");
}
