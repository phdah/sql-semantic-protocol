//! TASK-192 reviewer handoff completeness. The assignment queue is not a
//! substitute for independent full-document goldens or reviewer approval.
//! It must account for every fixed original parser cell before a reviewer
//! can make signed, commit-specific decisions.

use std::collections::BTreeSet;

use serde_json::Value;

const QUEUE: &str = include_str!("../docs/task-192-review-queue.json");
const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");
const LEDGER: &str = include_str!("../docs/task-192-semantic-cell-ledger.json");
const CANDIDATES: &str = include_str!("../docs/task-192-proposed-full-documents.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned TASK-192 semantic evidence")
}

fn pinned_commit(value: &Value) -> bool {
    value.as_str().is_some_and(|sha| {
        sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn accountable_reviewer(value: &Value) -> bool {
    value.as_str().is_some_and(|reviewer| !reviewer.trim().is_empty())
}

#[test]
fn reviewer_assignment_queue_names_every_original_cell_without_false_approval() {
    let queue = read(QUEUE);
    let freeze = read(FREEZE);
    let ledger = read(LEDGER);
    let candidate = read(CANDIDATES);

    assert_eq!(queue["schema_version"], 1);
    assert_eq!(queue["task_id"], "TASK-192");
    assert_eq!(queue["case_count"], 43);
    assert_eq!(queue["cell_count"], 559);
    assert_eq!(queue["parser_accepted"], 557);
    assert_eq!(queue["parser_rejected"], 2);
    // Review metadata is deliberately pending at creation time, but the test
    // must also admit a genuine, evidence-backed signoff transition. Otherwise
    // recording the required independent review would make CI permanently fail.
    let signed_off = queue["independent_signoff"]
        .as_bool()
        .expect("explicit overall review decision");
    if signed_off {
        assert!(accountable_reviewer(&queue["reviewer"]));
        assert!(pinned_commit(&queue["reviewed_commit"]));
        assert_eq!(candidate["independently_reviewed"], true);
        assert_eq!(ledger["independent_review_signed_off"], true);
    } else {
        assert!(queue["reviewer"].is_null());
        assert!(queue["reviewed_commit"].is_null());
        assert_eq!(candidate["independently_reviewed"], false);
        assert_eq!(ledger["independent_review_signed_off"], false);
    }

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
        assert_eq!(
            item["expected_full_canonical_document_ref"],
            format!("docs/task-192-proposed-full-documents.json#{id}"),
            "{id}: one full unreviewed document must be linked for the reviewer"
        );
        let proposed = candidate["cases"]
            .as_array()
            .expect("43 proposed documents")
            .iter()
            .find(|row| row["id"] == id)
            .expect("full document candidate");
        assert_eq!(proposed["original_sql"], original["sql"]);
        assert!(
            proposed["document"].is_object(),
            "{id}: a full wire document, not a partial shape"
        );
        assert!(candidate["independently_reviewed"].is_boolean());
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
    let mut approved_count = 0;
    for cell in actual_cells {
        let id = cell["id"].as_str().expect("review cell identity");
        assert!(seen.insert(id), "{id}: duplicate review cell");
        let expected = expected_cells
            .iter()
            .find(|expected| expected["id"] == id)
            .expect("review decision must match a frozen dialect cell");
        let evidence = cell["evidence_refs"]
            .as_array()
            .expect("explicit review evidence references");
        match cell["decision"].as_str().expect("review decision") {
            "pending" => {
                assert!(cell["reviewer"].is_null(), "{id}: pending review has no reviewer");
                assert!(
                    cell["reviewed_commit"].is_null(),
                    "{id}: pending review has no reviewed commit"
                );
                assert!(evidence.is_empty(), "{id}: pending review has no approval evidence");
                assert_eq!(expected["independently_reviewed"], false);
            }
            "approved" | "changes_requested" => {
                assert!(accountable_reviewer(&cell["reviewer"]), "{id}: missing reviewer");
                assert!(pinned_commit(&cell["reviewed_commit"]), "{id}: missing pinned commit");
                assert!(!evidence.is_empty(), "{id}: review needs supporting evidence");
                assert!(
                    evidence.iter().all(accountable_reviewer),
                    "{id}: review references must be nonempty"
                );
                let approved = cell["decision"] == "approved";
                assert_eq!(
                    expected["independently_reviewed"], approved,
                    "{id}: frozen semantic cell and reviewer decision disagree"
                );
                if approved {
                    approved_count += 1;
                }
            }
            other => panic!("{id}: unrecognized review decision {other}"),
        }
    }
    assert_eq!(
        seen, expected_ids,
        "all 559 fixed original cells require a review decision"
    );
    if signed_off {
        assert_eq!(
            approved_count, 559,
            "independent signoff requires all 559 independently reviewed cells"
        );
    }
}
