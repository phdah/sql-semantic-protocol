//! TASK-191 downstream reconciliation must preserve original SQL identities and
//! distinguish metadata parity from native execution or release approval.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const REVIEW: &str = include_str!("../docs/task-191-reviewed-dispositions-v2.json");
const HANDOFF: &str = include_str!("../docs/task-191-downstream-handoff-v3.json");
const TASK179: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
const TASK197: &str = include_str!("../docs/task-197-source-fact-matrix.json");
const SOURCES: &str = include_str!("../docs/task-177-parser-samples.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");

fn read(s: &str) -> Value {
    serde_json::from_str(s).expect("reviewed original source and downstream evidence")
}

fn rows<'a>(j: &'a Value, field: &str) -> &'a [Value] {
    j[field].as_array().expect("versioned evidence list")
}

fn label<'a>(j: &'a Value, field: &str) -> &'a str {
    j[field].as_str().expect("source-backed review label")
}

#[test]
fn every_reviewed_exception_reconciles_to_exact_original_source_and_existing_adapter_owners() {
    let reviewed = read(REVIEW);
    let handoff = read(HANDOFF);
    let vendor = read(TASK179);
    let adapter = read(TASK197);
    let samples = read(SOURCES);
    let manifest = read(MANIFEST);

    assert_eq!(handoff["schema_version"], 3);
    assert_eq!(handoff["task_id"], "TASK-191");
    assert_eq!(handoff["total_decision_cells"], 194);
    assert_eq!(handoff["reviewed_parser_rejected"], 55);
    assert_eq!(handoff["reviewed_unsupported"], 116);
    assert_eq!(handoff["reviewed_native_no_fixed_terminal"], 23);
    assert_eq!(handoff["task180_signoff"], false);
    assert_eq!(handoff["task88_release_approved"], false);
    assert_eq!(handoff["independent_external_vendor_runtime_proven"], false);
    let original_sql: BTreeMap<_, _> = rows(&samples, "cases")
        .iter()
        .map(|entry| (label(entry, "id"), label(entry, "sql")))
        .collect();
    let available_vendor: BTreeMap<&str, BTreeSet<&str>> = {
        let mut map = BTreeMap::<&str, BTreeSet<&str>>::new();
        for record in rows(&vendor, "cases") {
            if let Some(id) = record["source_case_id"].as_str() {
                map.entry(id).or_default().insert(label(record, "id"));
            }
        }
        map
    };
    let adapter_sources: BTreeMap<_, _> = rows(&adapter, "cases")
        .iter()
        .map(|item| (label(item, "id"), item))
        .collect();
    let mut expected_ids = BTreeSet::new();
    for (name, field) in [
        ("parser", "parser_cells"),
        ("unsupported", "unsupported_cells"),
        ("native", "native_original_cases"),
    ] {
        for item in rows(&reviewed, field) {
            let id = label(item, "original_id");
            let key = if name == "native" {
                format!("native/{id}")
            } else {
                format!("{name}/{}", label(item, "cell_id"))
            };
            assert!(expected_ids.insert(key));
        }
    }
    assert_eq!(expected_ids.len(), 194);
    let mut reviewed_ids = BTreeSet::new();
    let mut adapter_matched = BTreeSet::new();
    let mut vendor_matched = BTreeSet::new();
    for cell in rows(&handoff, "reviewed_decisions") {
        let key = label(cell, "review_id");
        let id = label(cell, "original_id");
        assert!(reviewed_ids.insert(key));
        assert_eq!(
            label(cell, "original_sql"),
            *original_sql.get(id).expect("stable original SQL fixture"),
            "{key}: cannot hand off a rewritten or companion SQL shape"
        );
        assert_eq!(cell["task180_final_owner"], "TASK-180");
        assert_eq!(cell["task88_parent"], "TASK-88");
        assert_eq!(cell["released_feature_support"], false);
        assert_eq!(cell["original_native_vendor_certified"], false);
        assert_eq!(cell["release_blocking"], true);
        assert_eq!(cell["vendor_runtime_attestation"], "not_attested");
        assert_eq!(
            cell["task178_handoff"],
            "docs/task-178-scope-split.json"
        );

        let expected_vendors = available_vendor.get(id).cloned().unwrap_or_default();
        let actual_vendors: BTreeSet<&str> = rows(cell, "task179_case_refs")
            .iter()
            .map(|v| v.as_str().expect("TASK-179 case identity"))
            .collect();
        assert_eq!(actual_vendors, expected_vendors);
        if !actual_vendors.is_empty() {
            vendor_matched.insert(id);
            assert_eq!(
                cell["task179_runtime_source"],
                "docs/task-179-independent-481-cell-signoff-v5.json"
            );
        } else {
            assert_eq!(cell["task179_runtime_source"], "no_task179_case_claim");
        }
        match adapter_sources.get(id) {
            Some(original) => {
                adapter_matched.insert(id);
                assert_eq!(
                    cell["task197_source_fact_file"],
                    "docs/task-197-source-fact-matrix.json"
                );
                assert_eq!(
                    cell["task197_source_fact_set"],
                    original["source_fact_set"]
                );
                assert_eq!(
                    cell["task197_source_fact_scope"],
                    original["metadata_scope"]
                );
                assert_eq!(
                    cell["task197_adapter_claim"],
                    "reviewed_source_facts_only_not_native_execution"
                );
            }
            None => {
                assert!(cell["task197_source_fact_file"].is_null());
                assert_eq!(cell["task197_source_fact_scope"], "no_matching_task197_source_fact_original");
            }
        }
    }
    assert_eq!(expected_ids, reviewed_ids);
    assert_eq!(handoff["task179_matched_original_cases"], vendor_matched.len());
    assert_eq!(handoff["task197_matched_original_cases"], adapter_matched.len());

    let scope = &manifest["task_191_boundary_disposition_acceptance"];
    assert_eq!(scope["reviewed_dispositions"], "docs/task-191-reviewed-dispositions-v2.json");
    assert_eq!(scope["downstream_handoff"], "docs/task-191-downstream-handoff-v3.json");
    assert_eq!(scope["full_matrix_release_approved"], false);
    assert_eq!(scope["all_release_blocking_original_features_implemented"], false);
}
