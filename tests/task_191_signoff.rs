//! TASK-191 bounded child signoff does not waive TASK-88 release obligations.

use serde_json::Value;

const REVIEW: &str = include_str!("../docs/task-191-independent-review-v4.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const QUEUE: &str = include_str!("../docs/backlog-child-queue-plan.json");
const TASK: &str = include_str!("../.backlog/tasks/task-191 - adjudicate-task-177-parser-and-native-boundaries.md");

#[test]
fn bounded_task191_acceptance_is_done_without_claiming_parent_or_vendor_release() {
    let review: Value = serde_json::from_str(REVIEW).expect("final task review");
    let manifest: Value = serde_json::from_str(MANIFEST).expect("reviewed coverage");
    let queue: Value = serde_json::from_str(QUEUE).expect("child queue");
    assert_eq!(review["schema_version"], 4);
    assert_eq!(review["child_task191_signed_off"], true);
    assert_eq!(review["reviewed_parser_cells"], 55);
    assert_eq!(review["reviewed_public_unsupported_cells"], 116);
    assert_eq!(review["reviewed_native_original_cases"], 23);
    assert_eq!(review["total_review_obligations"], 194);
    assert_eq!(review["unsupported_mandatory_feature_approved_as_deferred"], false);
    assert_eq!(review["native_non_duckdb_attested"], false);
    assert_eq!(review["parent_task88_signed_off"], false);
    assert_eq!(review["release_task91_approved"], false);
    for criterion in 1..=5 {
        assert!(
            review["acceptance_criteria"][format!("ac{criterion}")]
                .as_str()
                .is_some_and(|status| status.starts_with("signed_off_"))
        );
        assert!(
            TASK.contains(&format!("- [x] #{criterion} ")),
            "task checklist must be signed only with complete independently verified evidence"
        );
    }
    assert!(TASK.contains("status: Done"));
    let covered = &manifest["task_191_boundary_disposition_acceptance"];
    assert_eq!(covered["child_acceptance_criteria_signed_off"], true);
    assert_eq!(covered["all_release_blocking_original_features_implemented"], false);
    assert_eq!(covered["full_matrix_release_approved"], false);
    assert_eq!(covered["independent_review"], "docs/task-191-independent-review-v4.json");
    assert_eq!(
        covered["status"],
        "done_bounded_conditional_boundary_adjudication__parent_release_blocked"
    );
    let parent = queue["parents"]
        .as_array()
        .expect("tracked child queues")
        .iter()
        .find(|parent| parent["parent"] == 88)
        .expect("TASK-88 parent");
    let children = parent["phases"].as_array().expect("children");
    let item = children
        .iter()
        .find(|child| child["id"] == 191)
        .expect("TASK-191 in the queue");
    assert_eq!(item["acceptance_signoff"], "verified_bounded_child_pending_merge");
    assert_eq!(
        item["delivery_pr"],
        "https://github.com/phdah/sql-semantic-protocol/pull/205"
    );
}
