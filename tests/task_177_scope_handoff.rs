//! TASK-177 scoped completion must not silently waive the original TASK-88 release contract.

use serde_json::Value;

const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const QUEUE: &str = include_str!("../docs/backlog-child-queue-plan.json");
const PARSER: &str = include_str!("../docs/task-177-parser-snapshot.json");
const EQUIVALENCE: &str = include_str!("../docs/task-177-equivalence-snapshot.json");
const LEDGER: &str = include_str!("../docs/task-177-physical-proof-ledger.json");
const REMAINING: &str = include_str!("../docs/task-177-remaining-original-cases.json");

fn parse(contents: &str) -> Value {
    serde_json::from_str(contents).expect("versioned task-owned JSON")
}

#[test]
fn bounded_done_is_distinct_from_parent_semantic_and_release_approval() {
    let manifest = parse(MANIFEST);
    let scoped = &manifest["task_177_dialect_parser_acceptance"];
    let follow_up = &manifest["task_191_boundary_disposition_acceptance"];
    assert_eq!(scoped["status"], "done_bounded_parser_analyzer_inventory__full_semantic_acceptance_transferred");
    assert_eq!(scoped["child_done"], true);
    assert_eq!(scoped["full_ci_verified"], true);
    assert_eq!(scoped["scope_completion"]["scope_is_not_global_semantic_certification"], true);
    assert_eq!(scoped["historical_freeze_per_cell_release_certification_remains_unapproved"], true);
    for name in [
        "canonical_equivalence_certified_for_full_matrix",
        "typed_terminal_oracles_certified_for_full_matrix",
        "direct_dbt_odcs_parity_certified_for_full_matrix",
        "external_engine_laws_certified",
        "independent_review_verified",
        "parent_closed",
        "release_approved",
    ] {
        assert_eq!(scoped[name], false, "{name}: scoped Done cannot certify TASK-88");
    }
    assert_eq!(follow_up["status"], "to_do_release_blocking");
    assert_eq!(follow_up["parser_rejected_cells"], 55);
    assert_eq!(follow_up["analyzer_unsupported_cells"], 116);
    assert_eq!(follow_up["unproven_original_native_cases"], 23);
    assert_eq!(follow_up["full_matrix_release_approved"], false);

    let parser = parse(PARSER);
    let equivalence = parse(EQUIVALENCE);
    let ledger = parse(LEDGER);
    let remaining = parse(REMAINING);
    assert_eq!(scoped["variant_dialect_cells"], 3263);
    assert_eq!(scoped["parser_observed"]["parsed"], 3208);
    assert_eq!(scoped["parser_observed"]["parse_rejected"], 55);
    assert_eq!(parser["matrix"].as_object().expect("251 named masks").len(), 251);
    assert_eq!(equivalence["case_count"], 251);
    assert_eq!(scoped["canonical_equivalence_classifications"]["full_protocol_drifts"], 23);
    assert_eq!(ledger["native_duckdb_complete_original_sql_samples"], 228);
    assert_eq!(ledger["native_duckdb_not_proven_original_sql_samples"], 23);
    assert_eq!(remaining["case_count"], 23);
    assert_eq!(ledger["typed_sql_dbt_odcs_domain_parity_sample_count"], 10);
    assert_eq!(ledger["independent_signed_off_samples"], 0);
}

#[test]
fn task_88_child_queue_preserves_a_non_circular_final_acceptance_gate() {
    let queue = parse(QUEUE);
    let parent = queue["parents"]
        .as_array()
        .expect("parents")
        .iter()
        .find(|row| row["parent"] == 88)
        .expect("TASK-88 queue");
    let children: Vec<u64> = parent["phases"]
        .as_array()
        .expect("finite child queue")
        .iter()
        .map(|p| p["id"].as_u64().expect("numeric task ID"))
        .collect();
    assert_eq!(children, [177, 178, 179, 191, 180]);
    assert_eq!(parent["phases"][0]["scoped_status"], "Done (PR #196 merge pending)");
    assert_eq!(parent["phases"][3]["fixture_ids"], serde_json::json!(["88-C191-P", "88-C191-N"]));
    assert_eq!(parent["phases"][4]["id"], 180);
}
