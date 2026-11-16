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
    assert_eq!(review["approved_semantic_cells"], 402);
    assert_eq!(review["independently_adjudicated_semantic_cells"], 494);
    assert_eq!(review["reviewed_unapproved_cells"], 92);
    assert_eq!(review["reviewed_conditional_canonical_C_cells"], 26);
    assert_eq!(review["approval_requires_exact_head_ci"], true);
    assert_eq!(review["vendor_execution_attested"], false);
    assert_eq!(review["parent_task88_approved"], false);
    assert_eq!(review["release_approved"], false);
    let mut cases = BTreeSet::new();
    let mut cells = BTreeSet::new();
    let mut c = 0;
    let mut r = 0;
    let mut u = 0;
    let mut portable = 0;
    let mut conditional = 0;
    for reviewed in review["original_SQL_cases"]
        .as_array()
        .expect("38 reviewed laws")
    {
        let id = reviewed["id"].as_str().expect("stable SQL fixture");
        assert!(cases.insert(id), "{id}: no duplicate reviewed variant");
        let original = freeze["cases"]
            .as_array()
            .expect("frozen variants")
            .iter()
            .find(|candidate| candidate["id"] == id)
            .expect("review cannot invent original SQL");
        assert_eq!(
            reviewed["original_sql"], original["sql"],
            "{id}: original SQL drift"
        );
        assert_eq!(reviewed["feature_id"], original["feature_id"]);
        assert!(
            reviewed["independent_sql_law"]
                .as_str()
                .is_some_and(|law| law.len() >= 55),
            "{id}: a category label is not an independently reasoned SQL law"
        );
        assert_eq!(reviewed["portable_review_pending_final_head_ci"], true);
        assert!(reviewed["evidence_owner"].as_str().is_some());
        assert_eq!(reviewed["adapter_handoff_owner"], "TASK-197");
        assert_eq!(reviewed["final_matrix_signoff_owner"], "TASK-180");
        if reviewed["feature_id"]
            .as_str()
            .expect("family")
            .starts_with("dml.")
            && reviewed["physical_native_class"] == "original_native_dml_complete_target_bag"
        {
            assert!(
                write["cases"]
                    .as_array()
                    .expect("independent physical ordinals")
                    .iter()
                    .any(|entry| entry["id"] == id),
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
            assert!(
                cells.insert(format!("{id}/{name}")),
                "duplicate review cell"
            );
            assert_eq!(cell["native_vendor_attested"], false);
            assert!(cell["evidence"].as_str().is_some());
            assert_eq!(cell["source_type_authority"], "SQL_schema_or_authoritative_dbt_catalog_manifest_ODCS_only");
            let status = cell["original_observed_status"].as_str().expect("status");
            let classification = cell["reviewer_classification"].as_str().expect("review class");
            let expected = match status {
                "C" if id.ends_with("S198") => {
                    conditional += 1;
                    assert_eq!(cell["semantic_owner"], "TASK-179");
                    "reviewed_unapproved_conflict_order"
                }
                "C" if id.ends_with("S208") => {
                    conditional += 1;
                    assert_eq!(cell["semantic_owner"], "TASK-179");
                    "reviewed_unapproved_cascade_action"
                }
                "C" => {
                    portable += 1;
                    assert_eq!(cell["semantic_owner"], "TASK-196");
                    "reviewed_bounded_portable_canonical_under_frozen_source"
                }
                "R" => {
                    r += 1;
                    assert_eq!(cell["semantic_owner"], "TASK-191");
                    "reviewed_unapproved_parser_rejection"
                }
                "U" => {
                    u += 1;
                    assert!(cell["semantic_owner"].as_str().expect("owned gap").starts_with("TASK-"));
                    "reviewed_unapproved_unsupported_analysis"
                }
                other => panic!("{id}/{name}: unexpected historical status {other}"),
            };
            assert_eq!(classification, expected, "{id}/{name}: never approve an unproved semantic cell");
            assert_eq!(
                cell["final_acceptance"],
                if classification == "reviewed_bounded_portable_canonical_under_frozen_source" {
                    "bounded_portable_under_attested_source_bag_and_oracle"
                } else {
                    "unapproved_boundary_with_named_follow_up"
                }
            );
            if status == "C" {
                c += 1;
            }
        }
    }
    assert_eq!(cases.len(), 38);
    assert_eq!(cells.len(), 494);
    assert_eq!((c, r, u), (428, 11, 55));
    assert_eq!((portable, conditional), (402, 26));
    assert_eq!(portable + conditional + r + u, 494);
}
