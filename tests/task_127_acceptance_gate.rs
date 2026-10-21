//! TASK-127: permanent acceptance-evidence contract for the bounded QUALIFY child.
//! A recorded Exact outcome is never substituted by parser success or a Residual.
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
fn frozen_qualify_matrix_has_all_complete_evidence_and_owned_residuals() {
    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-127-acceptance-freeze.json")).unwrap();
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    let record = &manifest["task_127_qualify_acceptance"];
    let cases = frozen["cases"].as_array().unwrap();
    let dialects = frozen["required_dialects"].as_array().unwrap();
    let adapters = frozen["required_adapters"].as_array().unwrap();

    assert_eq!(frozen["owner"],"TASK-127");
    assert_eq!(frozen["parent"],"TASK-74");
    assert_eq!(frozen["status"],"verified_bounded_child");
    assert!(frozen["mandatory_unverified"].as_array().unwrap().is_empty());
    assert_eq!(record["status"],"verified_bounded_child");
    assert_eq!(record["task_127_child_acceptance_criteria_verified"],true);
    assert_eq!(record["all_original_parent_criteria_verified"],false);
    assert_eq!(record["release_approved"],false);
    assert_eq!(cases.len(),8);
    assert_eq!(dialects.len(),13);
    assert_eq!(adapters.len(),3);
    assert_eq!(cases.len()*dialects.len(),104);
    assert_eq!(cases.len()*adapters.len(),24);

    let expected = [
        "74-C127-P1","74-C127-P2","74-C127-P3","74-C127-P4",
        "74-C127-P5","74-C127-P6","74-C127-N1","74-C127-N2",
    ];
    let actual = cases.iter().map(|case|case["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual,expected.into_iter().collect::<BTreeSet<_>>());

    let mut exact = 0;
    let mut residual = 0;
    for case in cases {
        let id = case["id"].as_str().unwrap();
        for dialect in dialects {
            let name = dialect.as_str().unwrap();
            assert_eq!(case["parser_status_by_dialect"][name],"parsed","{id}/{name}");
        }
        let is_exact = case["proof"] == "mandatory_exact";
        if is_exact {
            exact+=1;
            assert_eq!(case["oracle_status"],"verified_independent_duckdb","{id}");
        } else {
            residual+=1;
            assert_eq!(id,"74-C127-N2");
            assert_eq!(case["oracle_status"],"conditional_without_unique_sort_tiebreaker");
        }
        for adapter in adapters {
            let name = adapter.as_str().unwrap();
            assert_eq!(
                case["adapter_status"][name],
                if is_exact {"verified_exact"} else {"verified_residual"},
                "{id}/{name}",
            );
        }
        assert!(case["sql"].as_str().unwrap().contains("QUALIFY"),"{id}");
        assert!(!case["physical_source_bags"].as_object().unwrap().is_empty(),"{id}");
        assert!(case["expected_full_terminal_bag"].is_array(),"{id}");
        assert!(case["typed_terminal_domains"].is_object(),"{id}");
    }
    assert_eq!((exact,residual),(7,1));
    assert_eq!(record["verified_mandatory_exact_cases"],exact);
    assert_eq!(record["conditional_residual_cases"],residual);
    assert_eq!(record["verified_adapter_cells"],24);
    assert_eq!(record["verified_dialect_parser_cells"],104);
    let boundaries = frozen["conditional_inventory"].as_array().unwrap();
    assert_eq!(boundaries.len(),3);
    for case in boundaries {
        assert_eq!(case["owner"],"TASK-127");
        assert_eq!(case["release_review"],"reviewed_bounded_residual_pending_release");
        assert!(case["disposition"].as_str().unwrap().starts_with("Residual:"));
    }
}
