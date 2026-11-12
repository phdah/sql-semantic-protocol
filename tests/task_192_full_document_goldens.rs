//! Proposed TASK-192 full canonical document fixtures for every original
//! SQL shape and all public parser dialects. Whole-document snapshots are
//! pinned separately from independent source/typed/native SQL-law oracles;
//! a reviewer must still inspect/approve their semantic validity.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json};

const ORIGINAL: &str = include_str!("../docs/task-192-acceptance-freeze.json");
// Before a full-document candidate has been frozen this is an empty sentinel.
// No generated document should be considered independently authored by this
// test or by the companion (ignored) fixture generation helper.
const CANDIDATES: &str = include_str!("../docs/task-192-proposed-full-documents.json");

fn document(sql: &str, dialect: &str) -> Result<Value, sql_semantic_protocol::Error> {
    let parser = dialect_from_name(dialect).expect("public parser dialect");
    let protocol = analyze_sql(sql, dialect, parser.as_ref())?;
    let mut value: Value = serde_json::from_str(&to_json(&protocol))
        .expect("emitted canonical JSON must be valid");
    for input in value["inputs"]
        .as_array_mut()
        .expect("canonical inputs")
    {
        // Parser identity is context, not semantic meaning.
        input["dialect"] = Value::Null;
    }
    Ok(value)
}

#[test]
fn all_43_full_documents_match_explicit_proposed_goldens_over_559_parser_cells() {
    let original: Value = serde_json::from_str(ORIGINAL).expect("43 fixed original SQLs");
    let expected: Value = serde_json::from_str(CANDIDATES).expect("versioned candidate documents");
    assert_eq!(expected["schema_version"], 1);
    assert_eq!(expected["owner"], "TASK-192");
    assert_eq!(expected["case_count"], 43);
    assert_eq!(expected["independently_reviewed"], false);
    let goldens = expected["cases"].as_array().expect("43 pinned full documents");
    assert_eq!(goldens.len(), 43);

    let mut seen = BTreeSet::new();
    let mut count = 0;
    for case in original["cases"].as_array().expect("43 original SQL variants") {
        let id = case["id"].as_str().expect("stable original id");
        assert!(seen.insert(id), "{id}: duplicate original SQL");
        let golden = goldens.iter().find(|candidate| candidate["id"] == id)
            .expect("each original SQL requires one full candidate golden");
        let sql = case["sql"].as_str().expect("fixed original SQL");
        assert_eq!(golden["original_sql"], sql, "{id}: original SQL changed");
        let expected_document = golden["document"].as_object()
            .expect("full independently auditable canonical JSON, not just domain.kind");
        assert!(!expected_document.is_empty(), "{id}: empty document");
        for (index, dialect) in original["dialects"].as_array().expect("13 public parsers")
            .iter().enumerate()
        {
            let dialect = dialect.as_str().expect("dialect identity");
            let status = case["dialect_cells"][index]["observed_parser_canonical_status"]
                .as_str().expect("historical status");
            let found = document(sql, dialect);
            if id == "88-C177-S179" && matches!(dialect, "bigquery" | "generic") {
                assert!(found.is_err(), "{id}/{dialect}: named parser exception");
                assert_eq!(status, "R");
            } else {
                let wire = found.unwrap_or_else(|error| {
                    panic!("{id}/{dialect}: unexpected public parser failure: {error}");
                });
                assert_eq!(wire.as_object(), Some(expected_document),
                    "{id}/{dialect}: complete public canonical document differs from pinned candidate");
            }
            count += 1;
        }
    }
    assert_eq!(count, 559);
    assert_eq!(seen.len(), 43);
    assert!(goldens.iter().all(|item| seen.contains(item["id"].as_str().expect("golden id"))));
}

/// Diagnostic for inspecting the actual analyzer documents before a reviewer
/// validates the candidate goldens. NEVER treat this as independent evidence.
#[test]
#[ignore]
fn emit_candidate_goldens_for_separate_semantic_adjudication() {
    let original: Value = serde_json::from_str(ORIGINAL).expect("original SQL");
    for case in original["cases"].as_array().expect("original variants") {
        let id = case["id"].as_str().expect("id");
        let sql = case["sql"].as_str().expect("sql");
        let doc = document(sql, "ansi").expect("ANSI parses all 43 original SQLs");
        println!("TASK192_DOCUMENT_BEGIN:{id}:{}", doc);
    }
}
