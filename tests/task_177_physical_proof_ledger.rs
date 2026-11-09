//! TASK-177 evidence ledger must account for every original TASK-66 variant.
//! Native DuckDB output and exact typed-domain/adapter proof are separate gates.

use std::collections::BTreeSet;

use serde_json::Value;

const LEDGER: &str = include_str!("../docs/task-177-physical-proof-ledger.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const RELATIONAL: &str = include_str!("../docs/task-177-relational-bag-oracles.json");
const PREDICATES: &str = include_str!("../docs/task-177-predicate-oracles.json");
const DRIFTS: &str = include_str!("../docs/task-177-canonical-drift-boundaries.json");
const EXPANSION: &str = include_str!("../docs/task-177-native-expansion.json");
const WRITES: &str = include_str!("../docs/task-177-write-oracles.json");
const ENVIRONMENT: &str = include_str!("../docs/task-177-predicate-environment-oracles.json");
const SCHEMA_LIFECYCLE: &str = include_str!("../docs/task-177-schema-lifecycle-oracles.json");
const WINDOWS: &str = include_str!("../docs/task-177-window-oracles.json");
const FINAL_NATIVE: &str = include_str!("../docs/task-177-final-native-deterministic.json");

fn parse(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned evidence")
}

fn sample_ids(document: &Value, key: &str, id_key: &str) -> BTreeSet<String> {
    document[key]
        .as_array()
        .expect("evidence array")
        .iter()
        .map(|item| item[id_key].as_str().expect("case ID").to_string())
        .collect()
}

#[test]
fn all_task_177_native_and_typed_proof_gaps_are_explicitly_accounted_for() {
    let ledger = parse(LEDGER);
    let samples = parse(SAMPLES);
    let relational = parse(RELATIONAL);
    let predicates = parse(PREDICATES);
    let drifts = parse(DRIFTS);

    let ids = sample_ids(&samples, "cases", "id");
    assert_eq!(ids.len(), 251);
    assert_eq!(ledger["sample_count"], 251);
    assert_eq!(ledger["task_id"], "TASK-177");

    let mut verified = sample_ids(&relational, "cases", "id");
    verified.extend(sample_ids(&predicates, "cases", "id"));
    // Canonical document drift does not imply a native DuckDB execution proof.
    // The five named oracle IDs are independently checked against the exact SQL
    // physically executed by tests/task_177_cross_dialect_native_oracles.rs.
    let drift_oracles: BTreeSet<String> = drifts["native_duckdb_original_sql_oracle_case_ids"]
        .as_array()
        .expect("explicit original SQL native proof registry")
        .iter()
        .map(|id| id.as_str().expect("case ID").to_string())
        .collect();
    let drift_cases = sample_ids(&drifts, "cells", "case_id");
    assert_eq!(drift_oracles.len(), 5);
    assert!(drift_oracles.is_subset(&drift_cases));
    verified.extend(drift_oracles);
    assert_eq!(
        verified.len(),
        125,
        "original native oracle baseline changed"
    );
    assert!(verified.is_subset(&ids));

    let expansion = parse(EXPANSION);
    let writes = parse(WRITES);
    let environment = parse(ENVIRONMENT);
    let schema = parse(SCHEMA_LIFECYCLE);
    let windows = parse(WINDOWS);
    let mut additional = BTreeSet::new();
    for (document, key) in [
        (&expansion, "bag_cases"),
        (&expansion, "ordered_cases"),
        (&expansion, "multi_statement_cases"),
        (&writes, "cases"),
        (&environment, "customer_cases"),
        (&environment, "temporal_cases"),
        (&environment, "floating_cases"),
        (&schema, "cases"),
        (&windows, "integer_cases"),
        (&windows, "float_cases"),
    ] {
        let entries = document[key].as_array().expect("task-owned native oracles");
        for entry in entries {
            let id = entry["id"].as_str().expect("original sample ID");
            assert!(
                additional.insert(id.to_owned()),
                "{id}: duplicate new oracle"
            );
        }
    }
    let sort_id = environment["ordering_case"]["id"]
        .as_str()
        .expect("original NULL-ordering oracle");
    assert!(
        additional.insert(sort_id.to_owned()),
        "{sort_id}: duplicate NULL ordering oracle"
    );
    assert_eq!(additional.len(), 89, "new native proof scope changed");
    assert!(
        additional.is_disjoint(&verified),
        "do not double-count existing native proof"
    );
    assert!(
        additional.is_subset(&ids),
        "all evidence must come from TASK-66"
    );
    verified.extend(additional);
    assert_eq!(verified.len(), 214, "whole-bag proof coverage changed");
    let final_native = parse(FINAL_NATIVE);
    let mut deterministic = BTreeSet::new();
    for key in [
        "original_integer_selects",
        "original_scalar_floating",
        "original_scalar_rendered",
        "original_catalog_state",
    ] {
        for item in final_native[key]
            .as_array()
            .expect("independent native oracles")
        {
            let id = item["id"].as_str().expect("TASK-66 original ID");
            assert!(
                deterministic.insert(id.to_owned()),
                "{id}: duplicate deterministic proof"
            );
        }
    }
    assert_eq!(deterministic.len(), 13);
    assert!(deterministic.is_disjoint(&verified));
    assert!(deterministic.is_subset(&ids));
    verified.extend(deterministic);
    assert_eq!(
        verified.len(),
        227,
        "all native oracle owners accounted for"
    );
    assert_eq!(ledger["native_duckdb_complete_original_sql_samples"], 227);
    assert_eq!(ledger["native_duckdb_not_proven_original_sql_samples"], 24);
    for id in ["88-C177-S033", "88-C177-S086", "88-C177-S140"] {
        assert!(
            !verified.contains(id),
            "{id}: rejection is not an exact terminal bag"
        );
    }

    let typed: BTreeSet<_> = [
        "88-C177-S001",
        "88-C177-S003",
        "88-C177-S004",
        "88-C177-S005",
        "88-C177-S006",
        "88-C177-S007",
    ]
    .into_iter()
    .collect();
    assert_eq!(ledger["six_case_sql_dbt_odcs_domain_parity"], 6);
    assert_eq!(ledger["independent_signed_off_samples"], 0);

    let mut observed = BTreeSet::new();
    let mut native_count = 0;
    let mut typed_count = 0;
    for cell in ledger["cases"]
        .as_array()
        .expect("all 251 case obligations")
    {
        let id = cell["id"].as_str().expect("ID");
        assert!(observed.insert(id.to_owned()), "duplicate ledger cell {id}");
        let sample = samples["cases"]
            .as_array()
            .expect("source registry")
            .iter()
            .find(|source| source["id"] == id)
            .expect("every cell must originate from TASK-66");
        assert_eq!(cell["feature_id"], sample["feature_id"]);
        assert_eq!(cell["variant"], sample["variant"]);
        let proved = verified.contains(id);
        if proved {
            native_count += 1;
        }
        assert_eq!(
            cell["native_duckdb_original_sql"],
            if proved {
                "full_source_and_terminal_bag_oracle"
            } else {
                "not_proven"
            },
            "{id}: never promote missing native proof"
        );
        if typed.contains(id) {
            typed_count += 1;
        }
        assert_eq!(
            cell["canonical_typed_adapter_parity"],
            if typed.contains(id) {
                "six_case_sql_dbt_odcs_exact_domain"
            } else {
                "not_proven"
            },
            "{id}: never promote missing typed adapter equivalence"
        );
        assert_eq!(
            cell["independent_signoff"], "blocked",
            "{id}: no task-wide acceptance signoff exists"
        );
        if matches!(id, "88-C177-S054" | "88-C177-S055") {
            assert_eq!(
                cell["native_exact_syntax"],
                "duckdb_rejected_alternate_verified"
            );
            assert!(
                !proved,
                "{id}: equivalent alternate SQL does not prove the original SQL"
            );
        }
        if id == "88-C177-S160" {
            assert_eq!(
                cell["native_exact_syntax"],
                "duckdb_rejected_frozen_fetch_with_ties"
            );
            assert!(
                !proved,
                "{id}: a fixture-specific LIMIT alternative is not the original"
            );
        }
        if id == "88-C177-S083" {
            assert_eq!(
                cell["native_exact_syntax"],
                "duckdb_executes_canonical_name_alignment_unsupported"
            );
        }
    }
    assert_eq!(observed, ids, "no original SQL variant may be omitted");
    assert_eq!(native_count, 227);
    assert_eq!(typed_count, 6);
}
