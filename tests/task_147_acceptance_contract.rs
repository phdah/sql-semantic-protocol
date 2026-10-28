//! Reject incomplete TASK-147 acceptance inventories rather than signing off
//! a few passing examples or misclassifying parser success as native evidence.

use std::collections::BTreeSet;

#[test]
fn every_required_fixture_dialect_adapter_and_parent_mapping_is_frozen() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-147-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["version"], 2);
    assert_eq!(
        freeze["fixture_ids"],
        serde_json::json!(["80-C147-P", "80-C147-N"])
    );
    assert_eq!(
        freeze["previous_snapshot"],
        "docs/task-147-acceptance-freeze-v1.json"
    );
    assert_eq!(
        freeze["original_parent_ac"],
        serde_json::json!([1, 2, 3, 5, 6])
    );
    assert_eq!(freeze["task_ac"], serde_json::json!([1, 2, 3, 4, 5]));
    for ac in ["1", "2", "3", "4", "5"] {
        assert!(
            freeze["child_ac_mapping"][ac].is_object(),
            "missing TASK-147 AC {ac}"
        );
    }
    for ac in ["1", "2", "3", "5", "6"] {
        assert!(
            freeze["parent_ac_mapping"][ac].is_object(),
            "missing TASK-80 AC {ac}"
        );
    }
    let mandatory = freeze["mandatory_cases"].as_array().unwrap();
    let conditional = freeze["conditional_cases"].as_array().unwrap();
    assert_eq!(mandatory.len(), 5);
    assert_eq!(conditional.len(), 9);
    let all = mandatory.iter().chain(conditional);
    let mut ids = BTreeSet::new();
    let mut positive = 0;
    let mut negative = 0;
    for case in all {
        let id = case["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate case: {id}");
        match case["fixture_id"].as_str().unwrap() {
            "80-C147-P" => positive += 1,
            "80-C147-N" => negative += 1,
            other => panic!("foreign fixture ID: {other}"),
        }
        for field in [
            "sql",
            "source",
            "prestate",
            "proof",
            "proof_positive",
            "proof_not_true",
            "comparison_assumptions",
            "canonical_source_instances",
            "canonical_target_instances",
            "expected_native_source_after",
            "expected_native_target_after",
            "adapter_evidence",
            "release_disposition",
        ] {
            assert!(!case[field].is_null(), "{id} lacks {field}");
        }
        assert!(case["source"].is_array());
        assert!(case["prestate"].is_array());
        assert!(case["canonical_intermediate_states"].is_array());
        if case["class"] == "mandatory_exact" {
            for field in [
                "expected_poststate",
                "expected_returning",
                "typed_domains",
                "typed_returning_domains",
                "expected_affected",
                "sql_true_source_ids",
                "sql_not_true_source_ids",
            ] {
                assert!(!case[field].is_null(), "{id} lacks exact field {field}");
            }
        }
        if case["class"] == "conditional_residual" {
            assert_eq!(case["proof"], "Residual", "{id}");
            assert_eq!(case["owner"], "TASK-147", "{id}");
            assert!(
                case["expected_poststate"].is_null(),
                "{id}: native law is unknown"
            );
        }
    }
    assert!(positive > 0 && negative > 0);
    assert_eq!(ids.len(), 14);
    let dialects = freeze["dialects"].as_array().unwrap();
    assert_eq!(dialects.len(), 13);
    for dialect in dialects {
        let d = &freeze["dialect_cells"][dialect.as_str().unwrap()];
        assert_eq!(d.as_object().unwrap().len(), ids.len());
        for id in &ids {
            let cell = d[*id].as_str().unwrap();
            assert!(
                matches!(
                    cell,
                    "parser_boundary"
                        | "canonical_unsupported"
                        | "canonical_conflict"
                        | "canonical_returning"
                ),
                "{id}: unverified dialect classification {cell}"
            );
        }
    }
    assert_eq!(freeze["required_dialect_cells"], 182);
    for adapter in freeze["adapters"].as_array().unwrap() {
        let cells = &freeze["adapter_cells"][adapter.as_str().unwrap()];
        assert_eq!(cells.as_object().unwrap().len(), ids.len());
        for id in &ids {
            let evidence = &cells[*id];
            assert_eq!(evidence["owner"], "TASK-147");
            assert!(!evidence["status"].as_str().unwrap().contains("pending"));
            assert!(!evidence["evidence"].as_str().unwrap().is_empty());
            assert!(!evidence["disposition"].as_str().unwrap().is_empty());
        }
    }
    assert_eq!(freeze["required_adapter_cells"], 42);
}
