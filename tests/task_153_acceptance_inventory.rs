//! TASK-153 independent inventory integrity and original acceptance crosswalk.
//! A passing semantic oracle cannot silently compensate for a missing frozen cell.

use std::collections::BTreeSet;

use serde_json::{json, Value};

fn contract() -> Value {
    serde_json::from_str(include_str!("../docs/task-153-acceptance-freeze.json")).unwrap()
}

fn values(input: &Value) -> BTreeSet<String> {
    input
        .as_array()
        .expect("finite inventory")
        .iter()
        .map(|entry| entry.as_str().expect("stable inventory ID").to_owned())
        .collect()
}

#[test]
fn stable_frozen_variants_prove_every_original_child_and_parent_ac() {
    let freeze = contract();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    assert_eq!(freeze["version"], 3);
    assert_eq!(freeze["owner"], "TASK-153");
    assert_eq!(freeze["parent"], "TASK-82");
    assert_eq!(
        freeze["previous_snapshot"],
        "docs/task-153-acceptance-freeze-v2.json"
    );
    assert_eq!(freeze["task_ac"], json!([1, 2, 3, 4, 5]));
    assert_eq!(freeze["original_parent_ac"], json!([1, 2, 3, 5, 6]));
    let fixtures = values(&freeze["fixture_ids"]);
    assert_eq!(
        fixtures,
        BTreeSet::from(["82-C153-P".to_string(), "82-C153-N".to_string()])
    );

    let root = &manifest["task_153_matched_merge_acceptance"];
    assert_eq!(root["owner"], "TASK-153");
    assert_eq!(root["parent"], "TASK-82");
    assert_eq!(root["freeze_version"], 3);
    assert_eq!(root["freeze"], "docs/task-153-acceptance-freeze.json");
    assert_eq!(root["fixture_ids"], freeze["fixture_ids"]);
    assert_eq!(root["parser_cells_required"], 65);
    assert_eq!(root["adapter_cells_required"], 24);
    assert_eq!(root["ordered_replay_cases"], 1);
    assert_eq!(root["child_signed_off"], freeze["signoff"]["child_done"]);
    assert_eq!(root["parent_signed_off"], false);
    assert_eq!(root["release_approved"], false);

    let mut ids = BTreeSet::new();
    let mut exact = 0;
    let mut impossible = 0;
    let mut residual = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate frozen case {id}");
        assert!(fixtures.contains(case["fixture_id"].as_str().unwrap()));
        assert_eq!(case["case_owner"], "TASK-153");
        assert!(case["sql"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(case["target_before"].is_array());
        assert!(case["comparison_law"].as_str().is_some());
        match case["proof"].as_str().unwrap() {
            "Exact" => {
                exact += 1;
                assert!(case["target_after"].is_array());
                assert!(case["source"].is_array());
                assert!(case["before_domains"].is_object());
                assert!(case["after_domains"].is_object());
                assert!(case["source_domains"].is_object());
                assert!(case["row_ids"].is_array());
                assert!(case["source_row_ids"].is_array());
                assert!(case["ordinals"].is_array());
                assert!(case["not_true_ordinals"].is_array());
                assert!(case["affected"].is_object());
            }
            "Impossible" => {
                impossible += 1;
                assert!(case["reason"].as_str().is_some());
            }
            "Residual" => {
                residual += 1;
                assert!(case["owner"] == "TASK-153");
                assert!(case["gap"].as_str().is_some());
                assert!(case["reason"].as_str().is_some());
            }
            other => panic!("unexpected proof class {other}"),
        }
    }
    assert_eq!((exact, impossible, residual), (5, 1, 2));

    let replay = &freeze["ordered_replay"];
    assert_eq!(replay["owner"], "TASK-153");
    assert_eq!(replay["fixture_id"], "82-C153-P");
    assert_eq!(replay["proof"], "Exact");
    assert_eq!(replay["steps"].as_array().unwrap().len(), 3);
    assert_eq!(replay["source_domains"].is_object(), true);
    for step in replay["steps"].as_array().unwrap() {
        assert!(step["before"].is_array());
        assert!(step["after"].is_array());
        assert!(step["before_domains"].is_object());
        assert!(step["after_domains"].is_object());
        assert!(step["affected_ordinals"].is_array());
    }
    for index in 1..3 {
        assert_eq!(replay["steps"][index]["before"], replay["steps"][index - 1]["after"]);
    }
    assert_eq!(replay["terminal"], replay["steps"][2]["after"]);

    let crosswalk = freeze["acceptance_map"].as_object().unwrap();
    assert_eq!(crosswalk.len(), 5);
    for ac in ["1", "2", "3", "4", "5"] {
        assert!(crosswalk[ac].as_array().is_some_and(|parts| !parts.is_empty()));
    }
}

#[test]
fn parser_and_conditional_cells_are_complete_without_false_native_certification() {
    let freeze = contract();
    let dialects = values(&freeze["dialects"]);
    assert_eq!(dialects.len(), 13);
    let map = freeze["dialect_expected"].as_object().unwrap();
    assert_eq!(map.len(), dialects.len());
    let mut exact = 0;
    let mut boundary = 0;
    for (dialect, cases) in map {
        assert!(dialects.contains(dialect));
        let classified = cases.as_array().unwrap();
        assert_eq!(classified.len(), 5);
        for (index, item) in classified.iter().enumerate() {
            if index < 3 {
                assert_eq!(item, "parsed_canonical");
                exact += 1;
            } else {
                assert_eq!(item, "parser_boundary");
                boundary += 1;
            }
        }
    }
    assert_eq!((exact, boundary), (39, 26));
    assert_eq!(freeze["dialect_evidence"]["canonical_cells"], 39);
    assert_eq!(freeze["dialect_evidence"]["conditional_parser_boundaries"], 26);
    assert_eq!(values(&freeze["adapters"]).len(), 3);
    assert_eq!(freeze["vendor_boundaries"].as_array().unwrap().len(), 2);
    for boundary in freeze["vendor_boundaries"].as_array().unwrap() {
        assert_eq!(boundary["owner"], "TASK-153");
        assert_eq!(boundary["proof"], "Residual");
        assert_eq!(boundary["parser_result"], "parser_boundary");
        assert_eq!(boundary["parser_cells"], 13);
        assert_eq!(boundary["native_execution_certified"], false);
        assert!(boundary["release_disposition"].as_str().is_some());
    }
    assert_eq!(freeze["signoff"]["parent_done"], false);
}
