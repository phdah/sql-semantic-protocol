//! TASK-179 independent bounded review of every mandatory dialect cell.
//!
//! Source SQL, exact native DuckDB oracle and strongest sound typed claim
//! are separate from an external vendor execution. A reviewed Residual is an
//! explicit limitation, not a claim of external vendor compatibility.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const FINAL: &str = include_str!("../docs/task-179-independent-481-cell-signoff-v5.json");
const MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
const EVIDENCE: &str = include_str!("../docs/task-179-source-and-outcome-evidence-v4.json");
const RECONCILIATION: &str = include_str!("../docs/task-179-reconciled-proofs-v3.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

fn read(text: &str) -> Value {
    serde_json::from_str(text).expect("task-owned review and frozen native evidence")
}

#[test]
fn all_481_acceptance_cells_have_exact_source_typed_outcome_and_conditional_vendor_dispositions() {
    let final_review = read(FINAL);
    let matrix = read(MATRIX);
    let evidence = read(EVIDENCE);
    let inherited = read(RECONCILIATION);
    let coverage = read(COVERAGE);
    assert_eq!(final_review["schema_version"], 5);
    assert_eq!(final_review["task_id"], "TASK-179");
    assert_eq!(final_review["cases"], 37);
    assert_eq!(final_review["reviewed_dialect_cells"], 481);
    assert_eq!(final_review["native_non_duckdb_attested"], false);
    assert_eq!(final_review["native_external_vendor_signoff"], false);
    assert_eq!(final_review["parent_task88_signoff"], false);
    assert_eq!(
        coverage["task_179_vendor_session_laws"]["independent_481_review"],
        "docs/task-179-independent-481-cell-signoff-v5.json"
    );
    assert_eq!(
        coverage["task_179_vendor_session_laws"]["physical_source_outcomes"],
        "docs/task-179-source-and-outcome-evidence-v4.json"
    );
    let cases = matrix["cases"].as_array().expect("37 original and identifier laws");
    let physical = evidence["cases"].as_array().expect("complete typed source oracles");
    let proofs = inherited["cases"].as_array().expect("reviewed original SQL owners");
    let dialects = matrix["dialects"].as_array().expect("13 exposed dialects");
    let decisions = final_review["decisions"].as_array().expect("all frozen dialect cells");
    assert_eq!(cases.len(), 37);
    assert_eq!(physical.len(), 37);
    assert_eq!(proofs.len(), 37);
    assert_eq!(decisions.len(), cases.len() * dialects.len());

    let mut distinct = BTreeSet::new();
    let mut observed = BTreeMap::<&str, usize>::new();
    let mut native_duckdb = 0;
    for (index, review) in decisions.iter().enumerate() {
        let case = &cases[index / dialects.len()];
        let source = &physical[index / dialects.len()];
        let inherited = &proofs[index / dialects.len()];
        let dialect = &case["dialect_cells"][index % dialects.len()];
        let identity = review["cell_id"].as_str().expect("unique fixture");
        let name = dialect["dialect"].as_str().expect("dialect");
        let id = case["id"].as_str().expect("case");
        assert!(distinct.insert(identity), "duplicated reviewed cell {identity}");
        assert_eq!(identity, format!("{id}/{name}"));
        assert_eq!(review["case_id"], case["id"]);
        assert_eq!(review["original_id"], case["source_case_id"]);
        assert_eq!(review["sql"], case["sql"]);
        assert_eq!(review["sql"], source["sql"]);
        assert_eq!(review["sql"], inherited["sql"]);
        assert_eq!(review["family"], case["family"]);
        assert_eq!(review["variant"], case["variant"]);
        assert_eq!(review["original_parser_status"], dialect["parser_status"]);
        assert_eq!(review["canonical_analysis_status"], dialect["analyzer_status"]);
        assert_eq!(review["source_review_owner"], inherited["source_semantic_owner"]);
        assert_eq!(review["source_columns"], source["source_columns"]);
        assert_eq!(review["source_population_identity"], source["source_instance_basis"]);
        assert_eq!(review["output_domain_policy"], source["typed_output_claim"]);
        assert_eq!(review["complete_terminal_disposition"], source["terminal_outcome"]);
        assert_eq!(review["source_fact_adapter_status"], case["source_adapter_provenance"]);
        assert_eq!(review["native_external_vendor_attested"], false);
        assert_eq!(review["vendor_runtime_proof"], "Residual");
        assert_eq!(
            review["task179_review_acceptance"],
            "approved_bounded_source_semantics_and_explicit_conditional_runtime_residual"
        );
        assert!(
            review["vendor_missing_attestation"]["engine_version"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        );
        assert!(
            review["vendor_missing_attestation"]["source"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        );
        let kind = review["parser_disposition"].as_str().expect("sound review decision");
        *observed.entry(kind).or_default() += 1;
        let expected = match dialect["analyzer_status"].as_str().expect("public outcome") {
            "analyzed" if case["family"] == "identifier_case" => {
                "task_owned_identifier_parser_analysis_verified"
            }
            "analyzed" => "canonical_analysis_accepted",
            "unsupported" => "canonical_unsupported_residual",
            "parser_rejected" => "parser_rejected_not_native_proof",
            other => panic!("{identity}: unreviewed semantic disposition {other}"),
        };
        assert_eq!(kind, expected, "{identity}: parser and review drift");
        if name == "duckdb" {
            native_duckdb += 1;
            assert!(review["native_original_sql_version"].is_string());
            assert_ne!(
                review["native_duckdb_disposition"],
                "native_engine_not_executed_conditional_residual"
            );
        } else {
            assert!(review["native_original_sql_version"].is_null());
            assert_eq!(
                review["native_duckdb_disposition"],
                "native_engine_not_executed_conditional_residual",
                "{identity}: no fabricated external execution"
            );
        }
        assert!(
            source["source_bag"].is_array(),
            "{identity}: source occurrence inventory must exist"
        );
        if source["exact_terminal_bag"].is_null() {
            assert!(
                matches!(
                    source["source_id"].as_str(),
                    Some("88-C177-S033" | "88-C177-S162")
                ),
                "{identity}: terminal absent without a reviewed native error or tie residual"
            );
        }
        let typed = &review["output_domain_policy"];
        assert!(typed["kind"].is_string(), "{identity}: absent typed domain");
        assert!(
            typed["lower"].is_null() || typed["lower_inclusive"].is_boolean(),
            "{identity}: unreviewed lower interval inclusivity"
        );
        assert!(
            typed["upper"].is_null() || typed["upper_inclusive"].is_boolean(),
            "{identity}: unreviewed upper interval inclusivity"
        );
    }
    assert_eq!(distinct.len(), 481);
    assert_eq!(native_duckdb, 37);
    assert_eq!(
        observed,
        BTreeMap::from([
            ("canonical_analysis_accepted", 416),
            ("canonical_unsupported_residual", 35),
            ("parser_rejected_not_native_proof", 4),
            ("task_owned_identifier_parser_analysis_verified", 26),
        ])
    );
}
