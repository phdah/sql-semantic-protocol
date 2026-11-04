//! TASK-82 parent acceptance registry: reconcile frozen child cases and gates.
//! Child execution oracles and parent DuckDB replay run as separate test targets.

use std::collections::BTreeSet;

use serde_json::Value;

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn child_freezes() -> [Value; 4] {
    [
        parse(include_str!("../docs/task-153-acceptance-freeze.json")),
        parse(include_str!("../docs/task-154-acceptance-freeze.json")),
        parse(include_str!("../docs/task-155-acceptance-freeze.json")),
        parse(include_str!("../docs/task-156-acceptance-freeze.json")),
    ]
}

#[test]
fn every_child_case_and_parent_ac_is_accounted_for_without_reclassification() {
    let freeze = parse(include_str!(
        "../docs/task-82-parent-acceptance-freeze.json"
    ));
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["owner"], "TASK-82");
    assert_eq!(freeze["independent_parent_signoff"], false);
    let children = child_freezes();
    let indexed = freeze["merged_child_inventory"].as_object().unwrap();
    assert_eq!(indexed.len(), children.len());
    let mut ids = BTreeSet::new();
    let mut cases = 0;
    let mut strengths = [0usize; 3];
    let mut parent_ac_coverage = BTreeSet::new();
    let mut all_dialects: Option<Value> = None;

    for (index, child) in children.iter().enumerate() {
        let task = format!("TASK-{}", index + 153);
        assert_eq!(child["owner"], task);
        assert_eq!(child["parent"], "TASK-82");
        let entry = &indexed[&task];
        assert_eq!(entry["version"], child["version"]);
        assert_eq!(entry["fixture_ids"], child["fixture_ids"]);
        assert_eq!(entry["original_parent_ac"], child["original_parent_ac"]);
        assert_eq!(entry["dialects"], child["dialects"]);
        assert_eq!(
            entry["parser_cells"].as_u64().unwrap(),
            child["dialect_cells_required"].as_u64().unwrap()
        );
        let child_adapter_cases = child["adapter_cases_required"]
            .as_u64()
            .or_else(|| child["adapter_cells_required"].as_u64())
            .unwrap();
        assert_eq!(
            entry["adapter_cases"].as_u64().unwrap(),
            child_adapter_cases
        );
        assert_eq!(child["dialects"].as_array().unwrap().len(), 13);
        if let Some(dialects) = &all_dialects {
            assert_eq!(dialects, &child["dialects"]);
        } else {
            all_dialects = Some(child["dialects"].clone());
        }

        let actual = child["cases"].as_array().unwrap();
        let frozen = entry["cases"].as_array().unwrap();
        assert_eq!(actual.len(), frozen.len(), "{task}: exact case inventory");
        for (case, contract) in actual.iter().zip(frozen) {
            let id = case["id"].as_str().unwrap();
            assert_eq!(contract["id"], id);
            let strength = case["classification"]
                .as_str()
                .or_else(|| case["proof"].as_str())
                .unwrap();
            assert_eq!(contract["classification"], strength, "{id}");
            assert!(
                child["fixture_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|root| id.starts_with(root.as_str().unwrap())),
                "{id}: missing original child fixture root"
            );
            assert!(ids.insert(id.to_owned()), "{id}: duplicate identity");
            match strength {
                "Exact" => strengths[0] += 1,
                "Impossible" => strengths[1] += 1,
                "Residual" => strengths[2] += 1,
                other => panic!("unknown proof classification {other}"),
            }
            cases += 1;
        }
        for criterion in child["original_parent_ac"].as_array().unwrap() {
            parent_ac_coverage.insert(criterion.as_u64().unwrap());
        }
    }

    assert_eq!(cases, 38, "no new child case may be omitted");
    assert_eq!(strengths, [23, 5, 10], "never promote a child residual");
    assert_eq!(
        parent_ac_coverage,
        BTreeSet::from([1, 2, 3, 4, 5, 6]),
        "original TASK-82 AC #1..#6"
    );
    let mapping = freeze["parent_ac_mapping"].as_object().unwrap();
    assert_eq!(mapping.len(), 6);
    for criterion in 1..=6 {
        let key = criterion.to_string();
        let referenced = mapping[&key].as_array().unwrap();
        assert!(!referenced.is_empty(), "parent AC #{criterion}");
        for task in referenced {
            let task = task.as_str().unwrap();
            let child = &indexed[task];
            assert_eq!(
                child["original_parent_ac"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|ac| ac.as_u64() == Some(criterion)),
                true,
                "{task} cannot be attributed to parent AC #{criterion} without a freeze mapping"
            );
        }
    }
}

#[test]
fn parent_replay_inventory_and_active_coverage_manifest_are_consistent() {
    let freeze = parse(include_str!(
        "../docs/task-82-parent-acceptance-freeze.json"
    ));
    let manifest = parse(include_str!("../docs/coverage-manifest.json"));
    let parent = &manifest["task_82_parent_merge_closure_acceptance"];
    assert_eq!(parent["owner"], "TASK-82");
    assert_eq!(parent["freeze_version"], freeze["version"]);
    assert_eq!(
        parent["freeze"],
        "docs/task-82-parent-acceptance-freeze.json"
    );
    assert_eq!(
        parent["parent_signed_off"],
        freeze["independent_parent_signoff"]
    );
    let cases = freeze["cases"].as_array().unwrap();
    let replay = freeze["replay_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    assert_eq!(replay.len(), 2);
    assert_eq!(replay[0]["fixed_point"], true);
    assert_eq!(replay[1]["fixed_point"], false);
    let ids = cases
        .iter()
        .chain(replay)
        .map(|case| case["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 4);
    assert_eq!(
        ids,
        freeze["fixture_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect()
    );
    assert_eq!(parent["fixture_ids"], freeze["fixture_ids"]);

    for (task, key) in [
        ("TASK-153", "task_153_matched_merge_acceptance"),
        ("TASK-154", "task_154_unmatched_merge_acceptance"),
        ("TASK-155", "task_155_merge_conflict_acceptance"),
        ("TASK-156", "task_156_computed_sequential_merge_acceptance"),
    ] {
        assert_eq!(manifest[key]["child_signed_off"], true, "{task}");
        assert_eq!(manifest[key]["parent_signed_off"], false, "{task}");
        assert_eq!(
            freeze["merged_child_inventory"][task]["fixture_ids"].is_array(),
            true
        );
    }

    for case in replay {
        assert_eq!(case["classification"], "Exact");
        let steps = case["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["before"], case["initial"]["target"]);
        assert_eq!(steps[0]["after"], steps[1]["before"]);
        assert_eq!(steps[1]["after"], case["final"]["target"]);
        assert_eq!(case["initial"]["source"], case["final"]["source"]);
        for step in steps {
            assert!(step["sql"].as_str().unwrap().starts_with("MERGE INTO"));
            assert!(step["before_domains"]["id"]["frequencies"].is_object());
            assert!(step["before_domains"]["score"]["frequencies"].is_object());
            assert!(step["after_domains"]["id"]["frequencies"].is_object());
            assert!(step["after_domains"]["score"]["frequencies"].is_object());
            for action in ["inserted", "updated", "deleted"] {
                assert!(step["affected"][action].as_u64().is_some());
            }
        }
    }
}
