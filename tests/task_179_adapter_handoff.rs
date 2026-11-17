//! TASK-179 authoritative adapter source-fact ownership.
//!
//! Metadata adapters cannot attest session/runtime vendor laws. Reuse the
//! independently verified TASK-197 source-fact matrix where it exists, and
//! explicitly flag the four transaction scripts and two new identifier probes
//! that require task-owned evidence instead of inventing adapter parity.

use std::collections::BTreeSet;

use serde_json::Value;

const LAW_MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
const SOURCE_FACTS: &str = include_str!("../docs/task-197-source-fact-matrix.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

fn load(source: &str) -> Value {
    serde_json::from_str(source).expect("committed JSON evidence")
}

#[test]
fn each_conditional_vendor_law_preserves_authoritative_source_types_or_explains_absence() {
    let law = load(LAW_MATRIX);
    let facts = load(SOURCE_FACTS);
    let coverage = load(COVERAGE);
    assert_eq!(facts["task_id"], "TASK-197");
    assert_eq!(facts["case_count"], 163);
    assert_eq!(
        coverage["task_179_vendor_session_laws"]["adapter_handoff_test"],
        "tests/task_179_adapter_handoff.rs"
    );

    let source_cases = facts["cases"].as_array().expect("complete source-fact ledger");
    let cases = law["cases"].as_array().expect("complete TASK-179 law matrix");
    let mut total = 0;
    let mut representable = 0;
    let mut unrepresented = BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().expect("stable TASK-179 cell");
        let provenance = &case["source_adapter_provenance"];
        assert_eq!(provenance["execution"], "unattested");
        let original_id = case["source_case_id"].as_str();
        let source_case = original_id.and_then(|key| {
            source_cases
                .iter()
                .find(|entry| entry["id"].as_str() == Some(key))
        });
        if let Some(source) = source_case {
            representable += 1;
            assert_eq!(provenance["reuse"], "docs/task-197-source-fact-matrix.json");
            assert_eq!(provenance["owner"], source["owner"]);
            assert_eq!(provenance["original_sql_metadata_scope"], source["metadata_scope"]);
            assert_eq!(provenance["source_fact_set"], source["source_fact_set"]);
            assert_eq!(provenance["source_relations"], source["source_relations"]);
            assert_eq!(case["sql"], source["sql"]);
            for adapter in [
                "direct_sql",
                "dbt_manifest",
                "dbt_warehouse_catalog",
                "odcs",
            ] {
                assert_eq!(
                    provenance[adapter], "source_fact_type_only",
                    "{id}: adapter may attest metadata, not vendor execution"
                );
                assert!(
                    source["source_authority"][adapter].as_str().is_some(),
                    "{id}: missing authoritative source adapter {adapter}"
                );
            }
            assert_eq!(
                provenance["equality_oracle"],
                "tests/task_197_exhaustive_adapter_parity.rs"
            );
        } else {
            let source_id = original_id.unwrap_or(id);
            assert!(
                source_id.starts_with("88-C177-S22") || case["family"] == "identifier_case",
                "{id}: unexpected source-fact omission {source_id}"
            );
            unrepresented.insert(id);
            assert_eq!(provenance["reuse"], Value::Null);
            assert_eq!(provenance["owner"], "TASK-179");
            for adapter in ["dbt_manifest", "dbt_warehouse_catalog", "odcs"] {
                assert_eq!(
                    provenance[adapter], "metadata_cannot_attest_transaction",
                    "{id}: no invented execution evidence"
                );
            }
        }
        total += 1;
    }
    assert_eq!(total, 37);
    assert_eq!(representable, 31);
    assert_eq!(unrepresented.len(), 6);
    assert_eq!(law["task197_represented_original_case_count"], 31);
    assert_eq!(law["task179_unrepresented_original_or_synthetic_case_count"], 6);
}
