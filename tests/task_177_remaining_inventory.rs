//! TASK-177 all remaining original SQL samples are owned, even when unprovable.
//!
//! A native execution candidate, a rejected statement, a nonterminating query,
//! and an unbounded stochastic result are different evidence categories.
//! This inventory does not mark them Done or assert engine laws from parsing.

use std::collections::BTreeSet;

use serde_json::Value;

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const LEDGER: &str = include_str!("../docs/task-177-physical-proof-ledger.json");
const REMAINING: &str = include_str!("../docs/task-177-remaining-original-cases.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned complete evidence")
}

#[test]
fn every_missing_original_sql_has_one_explicit_semantic_owner_and_proof_obligation() {
    let samples = parse(SAMPLES);
    let ledger = parse(LEDGER);
    let remaining = parse(REMAINING);
    assert_eq!(remaining["schema_version"], 1);
    assert_eq!(remaining["owner"], "TASK-177");
    assert_eq!(remaining["case_count"], 23);
    let all_samples = samples["cases"]
        .as_array()
        .expect("251 original SQL samples");
    let ledger_cases = ledger["cases"].as_array().expect("251 ledger rows");
    let gaps: BTreeSet<&str> = ledger_cases
        .iter()
        .filter(|cell| cell["native_duckdb_original_sql"] == "not_proven")
        .map(|cell| cell["id"].as_str().expect("original ID"))
        .collect();
    assert_eq!(gaps.len(), 23);

    let mut observed = BTreeSet::new();
    let mut kinds = std::collections::BTreeMap::new();
    for case in remaining["cases"].as_array().expect("all exact gaps") {
        let id = case["id"].as_str().expect("case ID");
        assert!(observed.insert(id), "{id}: duplicate missing sample");
        let source = all_samples
            .iter()
            .find(|source| source["id"] == id)
            .expect("must own an original TASK-66 SQL shape");
        assert_eq!(case["feature_id"], source["feature_id"]);
        assert_eq!(case["variant"], source["variant"]);
        assert_eq!(case["sql"], source["sql"]);
        assert_eq!(case["semantic_owner"], source["semantic_owner"]);
        let kind = case["proof_obligation"]
            .as_str()
            .expect("bounded proof kind");
        assert!(
            matches!(
                kind,
                "native_execution_candidate"
                    | "explicit_native_rejection_or_conditional_syntax"
                    | "recursive_nontermination"
                    | "engine_sample_nondeterminism"
                    | "runtime_nondeterminism"
            ),
            "{id}: unknown evidence category"
        );
        *kinds.entry(kind).or_insert(0usize) += 1;
        assert_eq!(case["strongest_typed_domain"], "not_proven");
        assert_eq!(case["sql_dbt_odcs_parity"], "not_proven");
        assert_eq!(case["native_vendor_session_equivalence"], "not_proven");
        assert_eq!(case["independent_disposition"], "blocked_not_approved");
    }
    assert_eq!(observed, gaps, "no outstanding SQL can silently disappear");
    assert_eq!(kinds.get("native_execution_candidate"), Some(&1));
    assert_eq!(
        kinds.get("explicit_native_rejection_or_conditional_syntax"),
        Some(&15)
    );
    assert_eq!(kinds.get("recursive_nontermination"), Some(&1));
    assert_eq!(kinds.get("engine_sample_nondeterminism"), Some(&5));
    assert_eq!(kinds.get("runtime_nondeterminism"), Some(&1));
}
