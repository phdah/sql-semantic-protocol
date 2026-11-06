//! TASK-163: independently guard immutable acceptance scope, source/terminal
//! oracles, parser matrix, conditional ownership and parent AC coverage.

use serde_json::Value;
use std::collections::BTreeSet;

const EXPECTED: [&str; 8] = [
    "84-C163-P-transactional-ddl-rollback",
    "84-C163-P-drop-rollback",
    "84-C163-P-ddl-outside-transaction",
    "84-C163-N-temp-table-lifetime",
    "84-C163-N-deferred-fk",
    "84-C163-N-concurrent-isolation",
    "84-C163-N-vendor-implicit-commit",
    "84-C163-N-vendor-transaction-option",
];

#[test]
fn mandatory_and_conditional_frozen_cells_have_complete_reviewable_scope() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-163-acceptance-freeze.json")).unwrap();
    let v1: Value =
        serde_json::from_str(include_str!("../docs/task-163-acceptance-freeze-v1.json")).unwrap();
    assert_eq!(v1["version"], 1);
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["previous_freeze"], "docs/task-163-acceptance-freeze-v1.json");
    assert_eq!(freeze["fixture_ids"], v1["fixture_ids"]);
    assert_eq!(freeze["cases"], v1["cases"], "frozen fixture variants must not be silently weakened");
    assert_eq!(freeze["dialects"], v1["dialects"]);
    assert_eq!(freeze["original_child_ac"], serde_json::json!([1, 2, 3, 4, 5]));
    assert_eq!(freeze["original_parent_ac"], serde_json::json!([1, 2, 3, 5, 6]));

    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), EXPECTED.len());
    let ids = cases.iter().map(|c| c["id"].as_str().unwrap()).collect::<BTreeSet<_>>();
    assert_eq!(ids, EXPECTED.into_iter().collect());
    assert_eq!(cases.iter().filter(|case| case["mandatory"] == true).count(), 3);
    assert_eq!(cases.iter().filter(|case| case["mandatory"] == false).count(), 5);
    for case in cases {
        assert!(case["sql"].as_array().is_some_and(|sql| !sql.is_empty()));
        assert!(!case["prestate"].is_null());
        assert!(!case["poststate"].is_null());
        assert!(!case["typed_domains"].is_null());
        assert!(!case["positive"].as_str().unwrap().is_empty());
        assert!(!case["not_true"].as_str().unwrap().is_empty());
        if case["mandatory"] == true {
            assert_eq!(case["expected_proof"], "Exact");
            assert!(!case["native_oracle"].as_str().unwrap().is_empty());
        } else {
            assert_eq!(case["expected_proof"], "Residual");
            assert_eq!(case["owner"], "TASK-163");
            assert!(!case["disposition"].as_str().unwrap().is_empty());
        }
    }
    let mapping = freeze["acceptance_mapping"].as_object().unwrap();
    for ac in 1..=5 {
        let entry = mapping.get(&ac.to_string()).unwrap().as_array().unwrap();
        assert!(!entry.is_empty());
    }
    for ac in [1, 2, 3] {
        let entry = mapping.get(&ac.to_string()).unwrap().as_array().unwrap();
        for id in entry.iter().filter_map(Value::as_str) {
            assert!(ids.contains(id), "original child AC maps to missing frozen fixture {id}");
        }
    }
}

#[test]
fn explicit_dialect_matrix_covers_13_names_and_all_eight_variants() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-163-acceptance-freeze.json")).unwrap();
    let matrix = freeze["dialect_parser_matrix_expected"].as_object().unwrap();
    assert_eq!(matrix.len(), 13);
    assert_eq!(freeze["dialect_parser_cells"], 104);
    for name in freeze["dialects"].as_array().unwrap() {
        let cases = &matrix[name.as_str().unwrap()];
        assert_eq!(cases.as_object().unwrap().len(), 8);
        assert_eq!(cases["create_table"], "create_table");
        assert_eq!(cases["drop_table"], "drop");
        assert_eq!(cases["temporary_table"], "create_table");
        assert_eq!(cases["defer_constraints"], "parser_boundary");
        assert_eq!(cases["commit_chain"], "commit");
    }
    for dialect in ["snowflake", "bigquery"] {
        assert_eq!(matrix[dialect]["begin"], "parser_boundary");
    }
    // Parser acceptance never justifies a cross-engine DDL rollback claim.
    assert_eq!(freeze["semantics"]["native_engine"], "duckdb ~1.10506.0");
}

#[test]
fn coverage_manifest_tracks_every_parent_child_and_conditional_gate() {
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let task = &manifest["task_163_transaction_session_acceptance"];
    assert_eq!(task["owner"], "TASK-163");
    assert_eq!(task["parent"], "TASK-84");
    assert_eq!(task["parent_closed"], false);
    assert_eq!(task["freeze"], "docs/task-163-acceptance-freeze.json");
    assert_eq!(task["fixture_ids"], serde_json::json!(["84-C163-P", "84-C163-N"]));
    assert_eq!(task["parent_original_ac"], serde_json::json!([1, 2, 3, 5, 6]));
    assert_eq!(task["conditional_owner"], "TASK-163");
}
