//! Immutable TASK-144 bounded acceptance contract accounting.
//! Guards against partial AC signoff when any named case, dialect, adapter,
//! source identity or executable oracle is accidentally omitted.

mod common;

use common::DIALECTS;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap()
}

#[test]
fn every_frozen_case_has_a_complete_typed_exact_or_residual_contract() {
    let freeze = frozen();
    let cases = freeze["cases"].as_array().unwrap();
    let expected = BTreeSet::from([
        "78-C144-P-REUSED-FILTER",
        "78-C144-P-FINITE-RECURSION",
        "78-C144-N-NULL-DUP",
        "78-C144-P-MULTISTEP",
        "78-C144-N-MULTISTEP-NULL",
        "78-C144-P-VIEW",
        "78-C144-N-RECURSIVE-CYCLE",
        "78-C144-N-LATERAL-SESSION",
        "78-C144-P-SHADOW",
    ]);
    assert_eq!(cases.len(), expected.len());
    assert_eq!(
        cases
            .iter()
            .map(|case| case["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["fixture_ids"], json!(["78-C144-P", "78-C144-N"]));
    assert_eq!(
        freeze["original_parent_ac_coverage"],
        json!([1, 2, 3, 4, 5, 6])
    );
    let mut mandatory = 0;
    let mut conditional = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert_eq!(case["owner"], "TASK-144", "{id}: owner");
        assert_eq!(case["required_dialect_names"].as_array().unwrap()
            .iter().map(|entry| entry.as_str().unwrap()).collect::<BTreeSet<_>>(),
            DIALECTS.iter().copied().collect::<BTreeSet<_>>(), "{id}: all exposed dialect identities");
        assert!(case["initial_bags"].is_object(), "{id}: initial physical bag");
        assert!(case["true_proof"].as_str().is_some(), "{id}");
        assert!(case["not_true_proof"].as_str().is_some(), "{id}");
        assert!(case["true_tuple"].is_object());
        assert!(case["not_true_tuple"].is_object());
        assert!(case["domains"].is_object());
        let dialects = case["parser_cells"].as_array().unwrap();
        assert_eq!(dialects.len(), DIALECTS.len(), "{id}: frozen parser matrix");
        assert_eq!(
            dialects
                .iter()
                .map(|cell| cell["dialect"].as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            DIALECTS.iter().copied().collect::<BTreeSet<_>>(),
            "{id}: duplicate or missing parser cell"
        );
        let adapters = case["adapter_cells"].as_array().unwrap();
        assert_eq!(
            adapters.len(),
            3,
            "{id}: three explicit adapter classifications"
        );
        assert_eq!(
            adapters
                .iter()
                .map(|cell| cell["adapter"].as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "direct_sql",
                "dbt_compiled_manifest_catalog",
                "odcs_equivalent_source_schema",
            ]),
            "{id}: missing adapter ownership"
        );
        match case["classification"].as_str().unwrap() {
            "mandatory_exact" => {
                mandatory += 1;
                assert_eq!(case["true_proof"], "Exact", "{id}: positive");
                assert_eq!(case["not_true_proof"], "Impossible", "{id}: absent");
                assert!(case["expected_terminal"].is_array(), "{id}: complete bag");
                assert!(
                    case["expected_producers"].is_object(),
                    "{id}: complete producer"
                );
                for (column, domain) in case["domains"].as_object().unwrap() {
                    assert!(
                        domain["null_allowed"].is_boolean(),
                        "{id}/{column}: nullability"
                    );
                    assert_eq!(domain["lower"]["inclusive"], true, "{id}/{column}: lower");
                    assert_eq!(domain["upper"]["inclusive"], true, "{id}/{column}: upper");
                    assert!(domain["lower"]["value"].is_i64(), "{id}/{column}");
                    assert!(domain["upper"]["value"].is_i64(), "{id}/{column}");
                }
            }
            "conditional_residual" => {
                conditional += 1;
                assert_eq!(case["true_proof"], "Residual", "{id}");
                assert_eq!(case["not_true_proof"], "Residual", "{id}");
                assert!(case["required_evidence"].is_string(), "{id}");
                assert!(case["release_disposition"].is_string(), "{id}");
            }
            other => panic!("{id}: unowned classification {other}"),
        }
    }
    assert_eq!((mandatory, conditional), (7, 2));
}

#[test]
fn coverage_manifest_never_confuses_child_evidence_with_parent_merge_gate() {
    let freeze = frozen();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let owner = &manifest["task_144_multistep_cte_acceptance"];
    assert_eq!(owner["owner"], "TASK-144");
    assert_eq!(owner["parent"], "TASK-78");
    assert_eq!(owner["freeze_version"], freeze["version"]);
    assert_eq!(owner["mandatory_exact_cases"], 7);
    assert_eq!(owner["conditional_residual_cases"], 2);
    assert_eq!(owner["parser_cells"], 126);
    assert_eq!(owner["adapter_cells"], 27);
    assert_eq!(owner["fixture_ids"], freeze["fixture_ids"]);
    assert_eq!(owner["parent_signed_off"], false);
    assert_eq!(owner["release_approved"], false);
    assert_eq!(freeze["parent_signoff"]["release_approved"], false);
    assert_eq!(
        freeze["parent_signoff"]["required_child_ids"],
        json!([141, 142, 143, 144])
    );
    assert!(owner["freeze_history"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e == "docs/task-144-acceptance-freeze-v1.json"));
}
