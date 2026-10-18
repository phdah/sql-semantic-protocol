//! Guards TASK-71 parent closure against silently reopening or weakening frozen evidence.
//! Semantic truth is independently exercised by task_116_parent_join_oracles.

use std::collections::BTreeSet;

use serde_json::{json, Value};

#[test]
fn parent_closure_preserves_every_original_cell_and_reviewed_boundary() {
    let original: Value =
        serde_json::from_str(include_str!("../docs/task-71-acceptance-freeze.json")).unwrap();
    let evidence: Value =
        serde_json::from_str(include_str!("../docs/task-116-acceptance-freeze.json")).unwrap();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();

    assert_eq!(
        original["signoff"]["original_acceptance_criteria"],
        json!([1, 2, 3, 4, 5, 6])
    );
    assert_eq!(original["signoff"]["release_approved"], false);
    assert_eq!(
        manifest["task_71_join_acceptance"]["all_original_criteria_verified"],
        true
    );
    assert_eq!(
        manifest["task_71_join_acceptance"]["release_approved"],
        false
    );
    assert_eq!(
        manifest["task_116_join_acceptance"]["release_approved"],
        false
    );
    assert_eq!(original["mandatory"].as_array().unwrap().len(), 15);
    assert_eq!(original["conditional"].as_array().unwrap().len(), 12);

    for kind in ["mandatory", "conditional"] {
        let originals = original[kind].as_array().unwrap();
        let witnessed = evidence[kind].as_array().unwrap();
        let mut ids = BTreeSet::new();
        assert_eq!(originals.len(), witnessed.len());
        for case in originals {
            let id = case["id"].as_str().unwrap();
            assert!(ids.insert(id), "duplicate original cell {id}");
            let proof = witnessed
                .iter()
                .find(|item| item["id"] == id)
                .unwrap_or_else(|| panic!("missing independent evidence for {id}"));
            assert_eq!(case["sql"], proof["original_sql"], "{id}: frozen SQL");
            assert_eq!(case["acceptance_criteria"], proof["acceptance_criteria"]);
            if kind == "mandatory" {
                assert_eq!(case["classification"], "exact_required");
                assert_eq!(
                    case["verification"],
                    "verified_independent_duckdb_full_bag_domain_identity_and_child_adapters"
                );
                assert_eq!(
                    case["expected"]["terminal_count"],
                    proof["expected_terminal_count"]
                );
                assert!(!proof["output_domains"].as_array().unwrap().is_empty());
                assert!(proof["identity_sql"].is_string());
                assert_eq!(proof["dialects"].as_array().unwrap().len(), 13);
            } else {
                assert_eq!(case["owner"], "TASK-116");
                assert_eq!(
                    case["verification"],
                    "reviewed_parser_and_portable_proof_with_native_vendor_runtime_uncertified"
                );
                assert_eq!(
                    case["release_disposition"],
                    "reviewed_portable_only_native_laws_not_certified"
                );
                assert_eq!(
                    proof["native_vendor_execution"],
                    "not_certified_by_portable_DuckDB_or_parser"
                );
                for dialect in evidence["dialect_identities"].as_array().unwrap() {
                    let name = dialect.as_str().unwrap();
                    let parser = &proof["dialect_classifications"][name]["parser"];
                    let observed = case["parser_classifications"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|entry| entry["dialect"] == name)
                        .unwrap_or_else(|| panic!("{id}: missing parent dialect {name}"));
                    assert_eq!(&observed["status"], parser, "{id}: {name}");
                }
            }
        }
    }
}
