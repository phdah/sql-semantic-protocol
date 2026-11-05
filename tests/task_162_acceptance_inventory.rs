//! TASK-162: fail CI when the independently frozen acceptance inventory,
//! dialect matrix, adapter boundaries or parent traceability drift.

use std::collections::BTreeSet;

use serde_json::Value;

#[test]
fn all_frozen_transaction_cases_and_parser_cells_have_signed_off_ownership() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-162-acceptance-freeze.json")).unwrap();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let entry = &manifest["task_162_transaction_acceptance"];
    assert_eq!(entry["owner"], "TASK-162");
    assert_eq!(entry["parent"], "TASK-84");
    assert_eq!(entry["parent_closed"], false);
    assert_eq!(entry["freeze_version"], freeze["version"]);
    assert_eq!(freeze["fixture_ids"], serde_json::json!(["84-C162-P", "84-C162-N"]));
    assert_eq!(
        freeze["v1_immutable_preimplementation_freeze"],
        "docs/task-162-acceptance-freeze-v1.json"
    );

    let cases = freeze["cases"].as_array().unwrap();
    let names = cases
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), cases.len());
    assert!(cases.iter().any(|case| case["family"] == "BEGIN_COMMIT"));
    assert!(cases.iter().any(|case| case["family"] == "BEGIN_ROLLBACK"));
    assert!(cases.iter().any(|case| case["family"] == "INVALID_BEGIN"));
    assert!(cases.iter().any(|case| case["family"] == "INVALID_COMMIT"));
    assert!(cases.iter().any(|case| case["family"] == "INVALID_ROLLBACK"));
    assert!(cases.iter().any(|case| case["family"] == "SAVEPOINT_RELEASE_ROLLBACK_TO"));
    assert!(cases.iter().any(|case| case["family"] == "CANONICAL_SQL_BOUNDARY"));

    let dialects = freeze["dialects"].as_array().unwrap();
    let matrix = freeze["dialect_parser_matrix_expected"].as_object().unwrap();
    assert_eq!(dialects.len(), 13);
    assert_eq!(matrix.len(), 13);
    let mut cells = 0;
    for dialect in dialects {
        let name = dialect.as_str().unwrap();
        let variants = matrix.get(name).unwrap().as_object().unwrap();
        assert_eq!(variants.len(), 7, "{name}");
        cells += variants.len();
    }
    assert_eq!(cells, 91);
    assert_eq!(freeze["dialect_parser_cells"], 91);
    assert_eq!(entry["dialect_parser_cells"], 91);
    assert_eq!(freeze["native_engine_only"], "duckdb");

    for ac in 1..=5 {
        assert!(
            freeze["task_ac_mapping"][ac.to_string()].as_array().is_some_and(|rows| !rows.is_empty()),
            "TASK-162 acceptance #{ac} missing"
        );
    }
    for ac in 2..=6 {
        let rows = freeze["parent_ac_mapping"][ac.to_string()].as_array().unwrap();
        assert!(!rows.is_empty(), "TASK-84 parent AC #{ac} missing");
        for id in rows {
            assert!(names.contains(id.as_str().unwrap()), "unknown frozen case {id}");
        }
    }
    assert_eq!(freeze["conditional_release_disposition"]["owner"], "TASK-162");
    assert_eq!(freeze["conditional_release_disposition"]["classification"], "Residual");
    assert!(freeze["native_oracles"]["adapter_parity"].as_str().is_some());
    assert!(freeze["native_oracles"]["positive_full_bag"].as_array().is_some());
    assert!(freeze["native_oracles"]["negative_impossible"].as_array().is_some());
}
