//! TASK-192 reviewer handoff completeness. The assignment queue is not a
//! substitute for independent full-document goldens or reviewer approval.
//! It must account for every fixed original parser cell before a reviewer
//! can make signed, commit-specific decisions.

use std::collections::BTreeSet;

use serde_json::Value;

const QUEUE: &str = include_str!("../docs/task-192-review-queue.json");
const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");
const LEDGER: &str = include_str!("../docs/task-192-semantic-cell-ledger.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned TASK-192 semantic evidence")
}

#[test]
fn reviewer_assignment_queue_names_every_original_cell_without_false_approval() {
    let queue = read(QUEUE);
    let freeze = read(FREEZE);
    let ledger = read(LEDGER);

    assert_eq!(queue["schema_version"], 1);
    assert_eq!(queue["task_id"], "TASK-192");
    assert_eq!(queue["case_count"], 43);
    assert_eq!(queue["cell_count"], 559);
    assert_eq!(queue["parser_accepted"], 557);
    assert_eq!(queue["parser_rejected"], 2);
    assert_eq!(queue["independent_signoff"], false);
    assert!(queue["reviewer"].is_null());
    assert!(queue["reviewed_commit"].is_null());

    let originals = freeze["cases"].as_array().expect("43 fixed original SQLs");
    let policies = ledger["cases"].as_array().expect("43 expected policies");
    let cases = queue["cases"]
        .as_array()
        .expect("43 review case references");
    assert_eq!((originals.len(), policies.len(), cases.len()), (43, 43, 43));

    let mut found_cases = BTreeSet::new();
    for item in cases {
        let id = item["case_id"].as_str().expect("stable case identifier");
        assert!(found_cases.insert(id), "{id}: duplicate review case");
        let original = originals
            .iter()
            .find(|row| row["id"] == id)
            .expect("fixed original SQL");
        assert!(original["sql"].is_string(), "{id}: missing original SQL");
        let expected = policies
            .iter()
            .find(|row| row["id"] == id)
            .expect("typed policy");
        assert_eq!(
            item["source_terminal_oracle"], expected["native_duckdb_oracle"],
            "{id}: review must link the actual native physical oracle"
        );
        assert!(
            item["expected_full_canonical_document_ref"].is_null(),
            "{id}: no analyzer-snapshot goldens may be mistaken for independent documents"
        );
        let owner = item["downstream_owner"].as_str();
        assert!(
            owner.is_none_or(|name| matches!(name, "TASK-179" | "TASK-191")),
            "{id}: unexpected unapproved boundary owner"
        );
    }

    let expected_cells = ledger["cells"].as_array().expect("559 expected cells");
    let actual_cells = queue["cells"].as_array().expect("559 review decisions");
    assert_eq!(actual_cells.len(), 559);
    let expected_ids = expected_cells
        .iter()
        .map(|cell| cell["id"].as_str().expect("ledger cell identity"))
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for cell in actual_cells {
        let id = cell["id"].as_str().expect("review cell identity");
        assert!(seen.insert(id), "{id}: duplicate review cell");
        assert_eq!(cell["decision"], "pending", "{id}: review has not happened");
        assert!(cell["reviewer"].is_null(), "{id}: no signed reviewer");
        assert!(
            cell["reviewed_commit"].is_null(),
            "{id}: no reviewed commit"
        );
        assert_eq!(
            cell["evidence_refs"]
                .as_array()
                .expect("review evidence references")
                .len(),
            0,
            "{id}: no fabricated review artifacts"
        );
    }
    assert_eq!(
        seen, expected_ids,
        "all 559 fixed original cells require a review decision"
    );
    assert!(expected_cells
        .iter()
        .all(|cell| cell["independently_reviewed"] == false));
}
