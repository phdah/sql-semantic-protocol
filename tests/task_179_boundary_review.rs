//! TASK-179 exact per-parser conservative review decisions.
//! A reviewed conditional or Unsupported boundary cannot become a claim of
//! external vendor runtime certification or a substitute for TASK-88 signoff.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const REVIEW: &str = include_str!("../docs/task-179-boundary-review.json");
const MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
const PROOFS: &str = include_str!("../docs/task-179-reconciled-proofs-v3.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

fn read(text: &str) -> Value {
    serde_json::from_str(text).expect("frozen task-owned evidence")
}

#[test]
fn every_dialect_law_has_individually_owned_review_without_execution_promotion() {
    let review = read(REVIEW);
    let matrix = read(MATRIX);
    let proofs = read(PROOFS);
    let coverage = read(COVERAGE);
    assert_eq!(review["task_id"], "TASK-179");
    assert_eq!(review["schema_version"], 1);
    assert_eq!(review["cases"], 37);
    assert_eq!(review["parser_dialect_cells"], 481);
    assert_eq!(review["external_vendor_runtime_attested"], false);
    assert_eq!(review["signed_off_task179"], false);
    assert_eq!(review["parent88_signed_off"], false);
    assert_eq!(
        coverage["task_179_vendor_session_laws"]["bounded_boundary_review"],
        "docs/task-179-boundary-review.json"
    );

    let decisions = review["decisions"].as_array().expect("481 cell decisions");
    let cases = matrix["cases"].as_array().expect("frozen 37 variants");
    let source_proofs = proofs["cases"].as_array().expect("37 source reviews");
    let dialects = matrix["dialects"]
        .as_array()
        .expect("13 dialect identities");
    assert_eq!(decisions.len(), cases.len() * dialects.len());
    let mut seen = BTreeSet::new();
    let mut counted = BTreeMap::<&str, usize>::new();
    for (index, decision) in decisions.iter().enumerate() {
        let case = &cases[index / dialects.len()];
        let dialect_cell = &case["dialect_cells"][index % dialects.len()];
        let id = case["id"].as_str().expect("law identity");
        let name = dialect_cell["dialect"].as_str().expect("dialect identity");
        assert!(seen.insert(format!("{id}/{name}")));
        assert_eq!(decision["cell_id"], format!("{id}/{name}"));
        assert_eq!(decision["reviewed_sql"], case["sql"]);
        assert_eq!(decision["source_id"], case["source_case_id"]);
        assert_eq!(decision["parser_status"], dialect_cell["parser_status"]);
        assert_eq!(decision["analysis_status"], dialect_cell["analyzer_status"]);
        assert_eq!(decision["native_non_duckdb_attested"], false);
        assert_eq!(decision["native_duckdb_original_attested"], false);
        assert_eq!(decision["consumer_execution_attested"], false);
        assert_eq!(
            decision["conditional_status"],
            "reviewed_unapproved_external_law"
        );
        assert_eq!(
            decision["release_decision"],
            "external_execution_claim_not_approved"
        );
        assert_eq!(
            decision["required_attestation"]["vendor_engine_version"],
            "not_connected_or_not_executed"
        );
        let source = &source_proofs[index / dialects.len()];
        assert_eq!(
            decision["audited_source_owner"],
            source["source_semantic_owner"]
        );
        assert_eq!(decision["typed_source_review"], source["source_review"]);
        assert!(decision["required_vendor_law"]
            .as_str()
            .is_some_and(|law| !law.is_empty()));
        let class = decision["decision"].as_str().expect("named disposition");
        *counted.entry(class).or_default() += 1;
        match dialect_cell["analyzer_status"].as_str().expect("status") {
            "parser_rejected" => assert_eq!(class, "parser_rejection"),
            "unsupported" => assert_eq!(class, "unsupported_canonical_statement"),
            "analysis_error" => assert_eq!(class, "analysis_error_boundary"),
            "analyzed" => assert_eq!(
                class,
                if id == "88-C179-036" || id == "88-C179-037" {
                    "task_owned_identifier_parser_analysis_verified"
                } else {
                    "bounded_portable_source_semantics_with_vendor_law_unattested"
                }
            ),
            other => panic!("{id}/{name}: missing decision for {other}"),
        }
    }
    assert_eq!(seen.len(), 481);
    assert_eq!(
        counted,
        BTreeMap::from([
            (
                "bounded_portable_source_semantics_with_vendor_law_unattested",
                416
            ),
            ("unsupported_canonical_statement", 35),
            ("parser_rejection", 4),
            ("task_owned_identifier_parser_analysis_verified", 26),
        ])
    );
}
