//! TASK-196 independently authored original SQL law review queue.
//!
//! Every original schema/write/lifecycle variant has a separately reasoned
//! SQL law, a complete 13-dialect classification, and explicit evidence owner.
//! This is not release signoff or an external vendor execution attestation.

use std::collections::BTreeSet;

use serde_json::Value;

const REVIEW: &str = include_str!("../docs/task-196-independent-semantic-review.json");
const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");
const OCCURRENCES: &str = include_str!("../docs/task-196-original-write-occurrences.json");

fn read(text: &str) -> Value {
    serde_json::from_str(text).expect("versioned task-owned evidence")
}

#[test]
fn all_494_original_cells_have_a_case_specific_sql_law_and_explicit_nonvendor_decision() {
    let review = read(REVIEW);
    let freeze = read(FREEZE);
    let write = read(OCCURRENCES);
    assert_eq!(review["schema_version"], 1);
    assert_eq!(review["owner"], "TASK-196");
    assert_eq!(review["original_sql_variants"], 38);
    assert_eq!(review["reviewed_sql_laws"], 38);
    assert_eq!(review["parser_cells"], 494);
    assert_eq!(
        review["observed_status_counts"],
        serde_json::json!({"C":428,"R":11,"U":55})
    );
    assert_eq!(review["approved_semantic_cells"], 0);
    assert_eq!(review["independently_adjudicated_semantic_cells"], 0);
    assert_eq!(review["vendor_execution_attested"], false);
    assert_eq!(review["parent_task88_approved"], false);
    assert_eq!(review["release_approved"], false);
    let mut cases = BTreeSet::new();
    let mut cells = BTreeSet::new();
    let mut c = 0;
    let mut r = 0;
    let mut u = 0;
    for reviewed in review["original_SQL_cases"].as_array().expect("38 reviewed laws") {
        let id = reviewed["id"].as_str().expect("stable SQL fixture");
        assert!(cases.insert(id), "{id}: no duplicate reviewed variant");
        let original = freeze["cases"].as_array().expect("frozen variants")
            .iter().find(|candidate| candidate["id"] == id)
            .expect("review cannot invent original SQL");
        assert_eq!(reviewed["original_sql"], original["sql"], "{id}: original SQL drift");
        assert_eq!(reviewed["feature_id"], original["feature_id"]);
        assert!(
            reviewed["independent_sql_law"].as_str()
                .is_some_and(|law| law.len() >= 55),
            "{id}: a category label is not an independently reasoned SQL law"
        );
        assert_eq!(reviewed["portable_review_pending_final_head_ci"], true);
        assert!(reviewed["evidence_owner"].as_str().is_some());
        if reviewed["feature_id"].as_str().expect("family").starts_with("dml.")
            && reviewed["physical_native_class"] == "original_native_dml_complete_target_bag"
        {
            assert!(
                write["cases"].as_array().expect("independent physical ordinals")
                    .iter().any(|entry| entry["id"] == id),
                "{id}: original DML review cannot omit independently selected occurrences"
            );
        }
        let variants = reviewed["cells"].as_array().expect("13 parser cells");
        assert_eq!(variants.len(), 13);
        for (index, cell) in variants.iter().enumerate() {
            let name = cell["dialect"].as_str().expect("dialect");
            assert_eq!(cell["id"], format!("{id}/{name}"));
            assert_eq!(cell["id"], original["cells"][index]["id"]);
            assert_eq!(
                cell["original_observed_status"],
                original["cells"][index]["original_observed_status"]
            );
            assert!(cells.insert(format!("{id}/{name}")), "duplicate review cell");
            assert_eq!(cell["native_vendor_attested"], false);
            assert!(cell["evidence"].as_str().is_some());
            match cell["original_observed_status"].as_str().expect("status") {
                "C" => {
                    assert_eq!(
                        cell["reviewer_classification"],
                        "portable_equivalence_candidate_pending_full_typed_effect_proof"
                    );
                    c += 1;
                }
                "R" => {
                    assert_eq!(
                        cell["reviewer_classification"],
                        "reviewed_unapproved_parser_rejection"
                    );
                    r += 1;
                }
                "U" => {
                    assert_eq!(
                        cell["reviewer_classification"],
                        "reviewed_unapproved_unsupported_analysis"
                    );
                    u += 1;
                }
                status => panic!("{id}/{name}: unexpected historical classifier {status}"),
            }
        }
    }
    assert_eq!(cases.len(), 38);
    assert_eq!(cells.len(), 494);
    assert_eq!((c, r, u), (428, 11, 55));
}
