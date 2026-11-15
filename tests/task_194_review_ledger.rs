//! TASK-194 independent signoff ledger: one reviewed original SQL decision
//! and one explicit parser/semantic disposition for each of 286 fixed cells.
//! CORRESPONDING parser rejection is not silently certified as a runtime law.

use std::collections::BTreeSet;

use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-194-acceptance-freeze.json");
const INDEPENDENT: &str = include_str!("../docs/task-194-independent-bag-review.json");
const REVIEW: &str = include_str!("../docs/task-194-independent-semantic-review.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

#[test]
fn all_22_original_cases_and_286_parser_cells_have_distinct_reviewed_evidence() {
    let frozen: Value = serde_json::from_str(FREEZE).expect("original 22-case freeze");
    let source: Value = serde_json::from_str(INDEPENDENT).expect("independent SQL tuple review");
    let review: Value = serde_json::from_str(REVIEW).expect("individual semantic review ledger");
    let coverage: Value = serde_json::from_str(COVERAGE).expect("canonical manifest");
    assert_eq!(review["task_id"], "TASK-194");
    assert_eq!(review["cases_reviewed"], 22);
    assert_eq!(review["parser_cells_reviewed"], 286);
    assert_eq!(review["portable_typed_cells_reviewed"], 260);
    assert_eq!(review["fail_closed_arity_cells_reviewed"], 13);
    assert_eq!(review["explicitly_unapproved_parser_boundaries"], 13);
    assert_eq!(review["original_SQL_native_source_terminal_cases"], 20);
    assert_eq!(review["adversarial_two_column_native_cases"], 9);
    assert_eq!(review["original_non_emitted_candidate_occurrences"], 106);
    assert_eq!(review["parent_task_88_approved"], false);
    assert_eq!(
        review["review_result"],
        "approved_for_TASK_194_bounded_scope_only"
    );
    assert_eq!(
        coverage["task_194_set_operator_acceptance"]["task_done"], true,
        "a reviewed child cannot remain in progress after all five gates"
    );
    assert_eq!(
        coverage["task_194_set_operator_acceptance"]["independently_verified_semantic_cells"],
        273
    );
    assert_eq!(
        coverage["task_194_set_operator_acceptance"]["pending_independent_review"],
        false
    );
    assert_eq!(
        coverage["task_194_set_operator_acceptance"]["task88_parent_approved"],
        false
    );
    assert_eq!(review["release_task_91_approved"], false);
    assert_eq!(review["external_vendor_execution"], false);
    assert_eq!(
        coverage["task_194_set_operator_acceptance"]["freeze"],
        "docs/task-194-acceptance-freeze.json"
    );

    let original = frozen["cases"].as_array().expect("fixed variants");
    let reviewed = review["cases"]
        .as_array()
        .expect("separate per-case SQL review");
    let physical = source["cases"]
        .as_array()
        .expect("independent source bag review");
    assert_eq!(original.len(), 22);
    assert_eq!(reviewed.len(), 22);
    assert_eq!(physical.len(), 22);
    let mut seen = BTreeSet::new();
    let mut total_rejected = 0;
    for case in reviewed {
        let id = case["case_id"].as_str().expect("original case ID");
        assert!(seen.insert(id), "{id}: duplicate review decision");
        let f = original
            .iter()
            .find(|c| c["id"] == id)
            .expect("original SQL");
        let p = physical
            .iter()
            .find(|c| c["case_id"] == id)
            .expect("physical SQL law");
        assert_eq!(
            case["original_sql"], f["sql"],
            "{id}: original SQL must not drift"
        );
        assert_eq!(
            case["semantic_law"], p["semantic_rule"],
            "{id}: independently derived law"
        );
        assert_eq!(
            case["expected_output_column_names"],
            f["expected_output_column_names"]
        );
        assert_eq!(
            case["expected_terminal_occurrences"],
            p["expected_terminal_occurrences"]
        );
        assert_eq!(
            case["expected_non_emitted_candidate_occurrences"],
            p["expected_rejected_candidate_occurrences"]
        );
        assert_eq!(
            case["source_arm_instances"]
                .as_array()
                .expect("physical arms")
                .len(),
            p["source_arms"].as_array().expect("independent arms").len(),
            "{id}: physical occurrence identities"
        );
        let disposition = case["review_disposition"]
            .as_str()
            .expect("reviewed classification");
        if id == "88-C177-S084" {
            assert_eq!(
                disposition,
                "explicit_unapproved_CORRESPONDING_parser_boundary_TASK_191"
            );
            assert!(p["independent_terminal_bag"].is_null());
        } else if id == "88-C177-S086" {
            assert_eq!(
                disposition,
                "approved_fail_closed_arity_without_fake_terminal"
            );
            assert!(p["independent_terminal_bag"].is_null());
        } else {
            assert_eq!(
                disposition,
                "reviewed_exact_original_SQL_typed_portable_semantics"
            );
            assert_eq!(
                case["expected_terminal_occurrences"],
                p["independent_terminal_bag"]
                    .as_array()
                    .expect("full bag")
                    .len() as u64
            );
            assert_eq!(
                case["expected_non_emitted_candidate_occurrences"],
                p["independent_non_emitted_candidate_bag"]
                    .as_array()
                    .expect("full rejected candidate bag")
                    .len() as u64
            );
            total_rejected += p["independent_non_emitted_candidate_bag"]
                .as_array()
                .expect("rejected occurrences")
                .len();
        }
    }
    assert_eq!(seen.len(), 22);
    assert_eq!(total_rejected, 106);

    let cells = review["cells"].as_array().expect("all 286 reviewed cells");
    assert_eq!(cells.len(), 286);
    let mut seen_cells = BTreeSet::new();
    let mut valid = 0;
    let mut impossible = 0;
    let mut parser_only = 0;
    for cell in cells {
        let key = cell["cell_id"].as_str().expect("fixed reviewed cell ID");
        assert!(seen_cells.insert(key), "{key}: duplicate parser cell");
        let case_id = cell["case_id"].as_str().expect("case owner");
        let f = original
            .iter()
            .find(|c| c["id"] == case_id)
            .expect("original SQL");
        let frozen_cell = f["cells"]
            .as_array()
            .expect("13 original parser dialects")
            .iter()
            .find(|source| source["id"] == key)
            .expect("canonical dialect cell");
        assert_eq!(
            cell["parser_observation"],
            frozen_cell["original_observed_status"]
        );
        assert_eq!(cell["parser_dialect"], frozen_cell["dialect"]);
        assert_eq!(cell["vendor_execution"], "unattested");
        assert_eq!(cell["pending_whole_matrix_owner"], "TASK-180");
        match cell["review_disposition"]
            .as_str()
            .expect("explicit decision")
        {
            "reviewed_bounded_typed_canonical_and_independent_duckdb" => {
                assert_eq!(cell["parser_observation"], "C");
                assert!(cell["typed_adapter_evidence"].is_string());
                assert_ne!(case_id, "88-C177-S086");
                valid += 1;
            }
            "reviewed_explicit_semantic_arity_impossibility" => {
                assert_eq!(case_id, "88-C177-S086");
                assert_eq!(cell["parser_observation"], "C");
                impossible += 1;
            }
            "unapproved_parser_only_rejection_handoff_TASK_191" => {
                assert_eq!(case_id, "88-C177-S084");
                assert_eq!(cell["parser_observation"], "R");
                parser_only += 1;
            }
            other => panic!("{key}: unsupported review decision {other}"),
        }
    }
    assert_eq!((valid, impossible, parser_only), (260, 13, 13));
    assert_eq!(seen_cells.len(), 286);
}
