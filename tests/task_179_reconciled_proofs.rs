//! TASK-179 inherited independently reviewed SQL/source facts.
//!
//! Previously reviewed bounded canonical/DuckDB cases are reused unchanged.
//! Their existence does not prove vendor dialect execution, session settings,
//! physical transaction guarantees or future TASK-88 signoff.

use std::collections::BTreeMap;

use serde_json::Value;

const LEDGER: &str = include_str!("../docs/task-179-reconciled-proofs-v3.json");
const MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
const REVIEW_192: &str = include_str!("../docs/task-192-independent-semantic-review.json");
const REVIEW_195: &str = include_str!("../docs/task-195-independent-semantic-review.json");
const REVIEW_196: &str = include_str!("../docs/task-196-independent-semantic-review.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

fn read(text: &str) -> Value {
    serde_json::from_str(text).expect("canonical immutable source review")
}

#[test]
fn every_vendor_law_has_exact_prior_typed_and_physical_review_or_named_transaction_boundary() {
    let ledger = read(LEDGER);
    let matrix = read(MATRIX);
    let review_192 = read(REVIEW_192);
    let review_195 = read(REVIEW_195);
    let review_196 = read(REVIEW_196);
    let coverage = read(COVERAGE);
    assert_eq!(ledger["schema_version"], 3);
    assert_eq!(ledger["task_id"], "TASK-179");
    assert_eq!(ledger["variant_matrix"], "docs/task-179-variant-matrix-v2.json");
    assert_eq!(ledger["external_vendor_execution_attested"], false);
    assert_eq!(
        coverage["task_179_vendor_session_laws"]["reconciled_proof_ledger"],
        "docs/task-179-reconciled-proofs-v3.json"
    );

    let cases = ledger["cases"].as_array().expect("exactly 37 source reviews");
    let matrix_cases = matrix["cases"].as_array().expect("37 law cases");
    assert_eq!(cases.len(), 37);
    assert_eq!(ledger["case_count"], cases.len());
    assert_eq!(ledger["cell_count"], cases.len() * 13);
    let mut owners = BTreeMap::<&str, usize>::new();
    for (review, original) in cases.iter().zip(matrix_cases) {
        let id = review["id"].as_str().expect("review ID");
        assert_eq!(review["id"], original["id"], "{id}: reordered vendor law");
        assert_eq!(review["original_id"], original["source_case_id"]);
        assert_eq!(review["sql"], original["sql"], "{id}: changed SQL spelling");
        assert_eq!(review["family"], original["family"]);
        assert_eq!(review["external_vendor_semantics"], "conditionally_unverified");
        assert_eq!(review["release_status"], "not_attested");

        let owner = review["source_semantic_owner"].as_str().expect("owned SQL review");
        *owners.entry(owner).or_default() += 1;
        let original_id = review["original_id"].as_str();
        match owner {
            "TASK-192" => {
                let case = review_192["cases"]
                    .as_array()
                    .expect("43 reviewed original cases")
                    .iter()
                    .find(|entry| entry["case_id"].as_str() == original_id)
                    .unwrap_or_else(|| panic!("{id}: missing independently reviewed source"));
                assert_eq!(review["sql"], case["original_sql"]);
                assert_eq!(
                    review["independent_semantic_expectation"],
                    case["independent_expectation"],
                    "{id}: independently frozen typed domain changed"
                );
                assert_eq!(review["source_native_oracle"], case["native_oracle"]);
                assert_eq!(review["execution_limit"], "unattested");
            }
            "TASK-195" => {
                let case = review_195["cases"]
                    .as_array()
                    .expect("36 reviewed original cases")
                    .iter()
                    .find(|entry| entry["case_id"].as_str() == original_id)
                    .unwrap_or_else(|| panic!("{id}: missing independently reviewed sort"));
                assert_eq!(review["sql"], case["original_sql"]);
                assert_eq!(review["independent_sql_law"], case["independent_sql_algebra_law"]);
                assert_eq!(review["expected_native_terminal_bag"], case["expected_native_terminal_bag"]);
                assert_eq!(review["expected_native_rejected_candidate_bag"], case["expected_native_rejected_candidate_bag"]);
                assert_eq!(review["public_output_laws"], case["public_output_laws"]);
                assert_eq!(review["physical_source_instance_ids"], case["physical_source_instance_ids"]);
                assert_eq!(review["native_disposition"], case["original_native_execution_disposition"]);
            }
            "TASK-196" => {
                let case = review_196["original_SQL_cases"]
                    .as_array()
                    .expect("38 reviewed DML cases")
                    .iter()
                    .find(|entry| entry["id"].as_str() == original_id)
                    .unwrap_or_else(|| panic!("{id}: missing independently reviewed write"));
                assert_eq!(review["sql"], case["original_sql"]);
                assert_eq!(review["independent_sql_law"], case["independent_sql_law"]);
                assert_eq!(review["native_proof_source"], case["evidence_owner"]);
                assert_eq!(review["physical_native_class"], case["physical_native_class"]);
                assert_eq!(review["parser_cells"], case["cells"].as_array().expect("13 cells").len());
            }
            "TASK-163" => {
                assert!(
                    matches!(
                        original_id,
                        Some("88-C177-S221")
                            | Some("88-C177-S222")
                            | Some("88-C177-S223")
                            | Some("88-C177-S224")
                    ),
                    "{id}: only the four transaction script exceptions may use TASK-163"
                );
                assert_eq!(
                    review["physical_native_class"],
                    "transaction-specific owner not inherited from TASK-197"
                );
            }
            "TASK-179" => {
                assert!(original_id.is_none());
                assert_eq!(review["family"], "identifier_case");
                assert_eq!(review["physical_native_class"], "bounded_DuckDB_probe");
            }
            other => panic!("{id}: unrecognized source owner {other}"),
        }
    }
    assert_eq!(
        owners,
        BTreeMap::from([
            ("TASK-192", 20),
            ("TASK-195", 5),
            ("TASK-196", 6),
            ("TASK-163", 4),
            ("TASK-179", 2),
        ])
    );
}
