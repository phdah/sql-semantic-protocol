//! TASK-193: independent original-SQL review decisions are complete,
//! nonduplicated and tied to executable source and public semantic evidence.
//! Named MSSQL LATERAL AST drift is deliberately not certified.

use std::collections::BTreeSet;

use serde_json::Value;

const REVIEW: &str = include_str!("../docs/task-193-independent-semantic-review.json");
const MATRIX: &str = include_str!("../docs/task-193-source-instance-matrix-v2.json");
const FREEZE: &str = include_str!("../docs/task-193-acceptance-freeze.json");

#[test]
fn every_bounded_review_decision_matches_independently_pinned_source_and_public_claims() {
    let review: Value = serde_json::from_str(REVIEW).expect("human-readable per-cell review");
    let matrix: Value = serde_json::from_str(MATRIX).expect("independent native source matrix");
    let freeze: Value = serde_json::from_str(FREEZE).expect("original SQL and parser freeze");
    assert_eq!(review["original_cases_reviewed"], 24);
    assert_eq!(review["parser_cells_reviewed"], 312);
    assert_eq!(review["bounded_canonical_cells_approved"], 311);
    assert_eq!(review["discrepancies"].as_array().expect("discrepancies").len(), 0);
    assert_eq!(review["parent_task_88_approved"], false);
    assert_eq!(review["release_task_91_approved"], false);

    let rows = matrix["rows"].as_array().expect("original SQL source rows");
    let originals = freeze["cases"].as_array().expect("immutable original SQL");
    let cases = review["cases"].as_array().expect("independent per-case review");
    assert_eq!(cases.len(), 24);
    assert_eq!(rows.len(), cases.len());

    let mut reviewed_cases = BTreeSet::new();
    for case in cases {
        let id = case["case_id"].as_str().expect("original review identity");
        assert!(reviewed_cases.insert(id), "{id}: duplicate review");
        let row = rows.iter().find(|row| row["case_id"] == id).expect("source matrix row");
        let original = originals.iter().find(|item| item["id"] == id).expect("original SQL");
        assert_eq!(row["original_sql"], original["sql"], "{id}: SQL drift");
        assert_eq!(
            case["independent_expected_join_kinds"], row["expected_public_join_kinds"],
            "{id}: independently reviewed join kind"
        );
        assert_eq!(
            case["independent_expected_physical_dependencies"], row["expected_physical_dependencies"],
            "{id}: independently reviewed source identity"
        );
        assert_eq!(
            case["independent_expected_projection_lineage"], row["expected_projection_lineage"],
            "{id}: independently reviewed lineage"
        );
        assert_eq!(row["projection_lineage_rule"], "exact", "{id}: no weak lineage floor");
        let count = case["independent_expected_terminal_occurrences"]
            .as_u64()
            .expect("literal expected occurrence count") as usize;
        assert_eq!(row["expected_original_terminal_bag"].as_array().expect("terminal bag").len(), count);
        assert_eq!(original["expected_terminal_bag"].as_array().expect("original terminal").len(), count);
        assert_eq!(case["independent_expected_source_instance_occurrences"], row["expected_source_instance_count"]);
        assert_eq!(row["expected_source_instance_bag"].as_array().expect("physical alias bag").len(), count);
    }

    let cells = review["cells"].as_array().expect("per-dialect review decisions");
    assert_eq!(cells.len(), 312);
    let mut reviewed_cells = BTreeSet::new();
    let mut approved = 0;
    let mut explicitly_unapproved = 0;
    for cell in cells {
        let id = cell["cell_id"].as_str().expect("reviewed parser cell");
        assert!(reviewed_cells.insert(id), "{id}: duplicate review decision");
        let case_id = cell["original_case"].as_str().expect("original case id");
        assert!(reviewed_cases.contains(case_id), "{id}: unknown original case");
        let original = originals.iter().find(|item| item["id"] == case_id).expect("original cell owner");
        let frozen = original["cells"].as_array().expect("original parser cells")
            .iter().find(|candidate| candidate["id"] == id).expect("frozen cell");
        assert_eq!(cell["parser_observation"], frozen["observed_status"], "{id}: parser boundary");
        assert_eq!(cell["source_instance_oracle"], case_id);
        assert_eq!(cell["source_complete_duckdb_terminal_oracle"], case_id);
        if id == "88-C177-S152/mssql" {
            assert_eq!(cell["parser_observation"], "D");
            assert_eq!(cell["review_disposition"], "explicit_unapproved_ast_drift_task_179_task_191");
            explicitly_unapproved += 1;
        } else {
            assert_eq!(cell["parser_observation"], "C", "{id}: original parser accepted");
            assert_eq!(cell["review_disposition"], "approved_bounded_canonical_parser_and_typed_semantics");
            approved += 1;
        }
    }
    assert_eq!(reviewed_cases.len(), 24);
    assert_eq!(reviewed_cells.len(), 312);
    assert_eq!(approved, 311);
    assert_eq!(explicitly_unapproved, 1);
}
