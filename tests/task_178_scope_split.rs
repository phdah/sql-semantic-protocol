//! TASK-178 scope split: exact variant ownership and unchanged release gates.
//!
//! This validates handoff accounting, not a typed semantic or native vendor
//! proof. No original mandatory SQL/dialect cell can disappear from TASK-180.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

const HANDOFF: &str = include_str!("../docs/task-178-scope-split.json");
const ORIGINAL: &str = include_str!("../docs/task-178-full-variant-inventory.json");
const QUEUE: &str = include_str!("../docs/backlog-child-queue-plan.json");
const COVERAGE: &str = include_str!("../docs/coverage-manifest.json");

const TASKS: &[(&str, &str, &str)] = &[
    (
        "TASK-192",
        "TASK-178",
        include_str!(
            "../.backlog/tasks/task-192 - certify-predicate-and-structural-type-outcomes.md"
        ),
    ),
    (
        "TASK-193",
        "TASK-192",
        include_str!(
            "../.backlog/tasks/task-193 - certify-join-and-correlated-source-instance-bags.md"
        ),
    ),
    (
        "TASK-194",
        "TASK-193",
        include_str!("../.backlog/tasks/task-194 - certify-set-operator-bags-and-alignment.md"),
    ),
    (
        "TASK-195",
        "TASK-194",
        include_str!(
            "../.backlog/tasks/task-195 - certify-window-grouping-and-ordering-outcomes.md"
        ),
    ),
    (
        "TASK-196",
        "TASK-195",
        include_str!("../.backlog/tasks/task-196 - certify-schema-write-and-ddl-state-outcomes.md"),
    ),
    (
        "TASK-197",
        "TASK-196",
        include_str!("../.backlog/tasks/task-197 - close-cross-adapter-typed-semantic-parity.md"),
    ),
];

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("versioned backlog evidence")
}

#[test]
fn exact_163_original_variants_are_owned_once_without_global_semantic_signoff() {
    let split = parse(HANDOFF);
    let original = parse(ORIGINAL);
    let coverage = parse(COVERAGE);
    assert_eq!(split["schema_version"], 1);
    assert_eq!(split["task_id"], "TASK-178");
    assert_eq!(split["parent_task"], "TASK-88");
    assert_eq!(split["variant_count"], 163);
    assert_eq!(split["parser_cells"], 2119);
    assert_eq!(split["reviewed_parser_dialects"], 13);
    assert_eq!(split["independently_certified_semantic_cells"], 0);
    assert_eq!(split["global_approval"], false);
    assert_eq!(
        split["completion"]["task178_bounded_baseline_signoff"],
        true
    );
    assert_eq!(
        split["completion"]["global_semantic_matrix_certified"],
        false
    );
    assert_eq!(
        split["completion"]["all_original_task178_ac_certified"],
        false
    );
    assert_eq!(split["completion"]["parent_task88_done"], false);
    assert_eq!(split["completion"]["release_approved"], false);
    assert_eq!(original["independently_signed_off_cells"], 0);
    assert_eq!(original["release_approved"], false);

    let ownership = split["category_owners"]
        .as_object()
        .expect("each source feature family");
    assert_eq!(
        ownership.len(),
        11,
        "all feature families have an explicit owner"
    );
    let original_families = original["counts_by_family"]
        .as_object()
        .expect("source family counts");
    let actual_families: BTreeSet<_> = ownership.keys().collect();
    let expected_families: BTreeSet<_> = original_families.keys().collect();
    assert_eq!(
        actual_families, expected_families,
        "no unowned or invented feature family"
    );

    let mut original_ids = BTreeSet::new();
    let mut case_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut dialect_cells = 0usize;
    for case in original["cases"]
        .as_array()
        .expect("complete original variants")
    {
        let id = case["id"]
            .as_str()
            .expect("stable TASK-177 source SQL identity");
        assert!(original_ids.insert(id), "original variant reused: {id}");
        let family = case["feature_id"]
            .as_str()
            .expect("original feature id")
            .split('.')
            .next()
            .expect("named family");
        let owner = ownership[family]
            .as_str()
            .expect("every original family has one task");
        assert!(
            TASKS.iter().any(|(task, _, _)| *task == owner),
            "{id}: invalid owner {owner}"
        );
        *case_counts.entry(owner.to_string()).or_default() += 1;
        let cells = case["dialect_cells"]
            .as_array()
            .expect("no implicit parser skip");
        assert_eq!(cells.len(), 13, "{id}: no missing dialect cell");
        dialect_cells += cells.len();
    }
    assert_eq!(original_ids.len(), 163);
    assert_eq!(dialect_cells, 2119);
    for (owner, _, _) in TASKS.iter().take(5) {
        assert_eq!(
            case_counts.get(*owner).copied().unwrap_or_default(),
            split["task_variant_counts"][*owner]
                .as_u64()
                .expect("frozen owner count") as usize,
            "{owner}: lost or duplicated original variants"
        );
    }
    assert_eq!(case_counts.len(), 5);
    assert_eq!(split["cross_family_adapter_parity"], "TASK-197");
    assert_eq!(split["vendor_session_laws"], "TASK-179");
    assert_eq!(split["parser_and_unsupported_dispositions"], "TASK-191");
    assert_eq!(split["final_independent_matrix_signoff"], "TASK-180");

    for original_ac in ["1", "2", "3", "4", "5"] {
        let owners = split["original_ac_handoff"][original_ac]
            .as_array()
            .expect("all original criteria must have explicit handoff");
        assert!(
            owners.iter().any(|task| task == "TASK-180"),
            "original AC #{original_ac} lost final independent reviewer"
        );
    }
    let baseline = &coverage["task_178_canonical_equivalence_acceptance"];
    assert_eq!(baseline["child_done"], true);
    assert_eq!(baseline["scoped_acceptance_criteria_signed_off"], true);
    assert_eq!(
        baseline["original_five_acceptance_criteria_signed_off"],
        false
    );
    assert_eq!(baseline["independent_review_verified"], false);
    assert_eq!(baseline["parent_done"], false);
    assert_eq!(baseline["release_approved"], false);
}

#[test]
fn new_tasks_and_existing_final_gates_form_one_acyclic_queue() {
    let plan = parse(QUEUE);
    let coverage = parse(COVERAGE);
    let task_192_done = coverage["task_192_predicate_type_acceptance"]["task_done"]
        .as_bool()
        .expect("bounded TASK-192 status");
    let parent = plan["parents"]
        .as_array()
        .expect("parent queues")
        .iter()
        .find(|parent| parent["parent"] == 88)
        .expect("TASK-88 queue");
    let phases = parent["phases"].as_array().expect("versioned finite queue");
    let ids: Vec<_> = phases
        .iter()
        .map(|phase| phase["id"].as_u64().expect("child ID"))
        .collect();
    assert_eq!(
        ids,
        vec![177, 178, 192, 193, 194, 195, 196, 197, 179, 191, 180]
    );
    assert_eq!(plan["next_unallocated_child_id"], 198);
    for (task, predecessor, content) in TASKS {
        assert!(
            content.contains(&format!("id: {task}")),
            "{task}: missing file identity"
        );
        let expected_status = if *task == "TASK-192" {
            if task_192_done {
                "status: Done"
            } else {
                "status: In Progress"
            }
        } else {
            "status: To Do"
        };
        assert!(
            content.contains(expected_status),
            "{task}: unexpected backlog lifecycle state"
        );
        assert!(
            content.contains(&format!("  - {predecessor}")),
            "{task}: missing dependency"
        );
        assert!(
            content.contains("TASK-88 release-blocking child"),
            "{task}: original parent linkage lost"
        );
    }
    let baseline = include_str!("../.backlog/tasks/task-178 - certify-canonical-equivalence-for-shared-dialect-ast-variants.md");
    assert!(
        baseline.contains("status: Done"),
        "bounded TASK-178 baseline must have its own checked AC"
    );
    let vendor = include_str!(
        "../.backlog/tasks/task-179 - classify-conditional-session-and-vendor-execution-laws.md"
    );
    assert!(
        vendor.contains("dependencies:\n  - TASK-197"),
        "vendor law must follow parity handoff"
    );
    let parent_task = include_str!(
        "../.backlog/tasks/task-88 - build-dialect-law-and-execution-conformance-matrix.md"
    );
    assert!(
        parent_task.contains("status: To Do"),
        "parent cannot be closed by baseline split"
    );
}
