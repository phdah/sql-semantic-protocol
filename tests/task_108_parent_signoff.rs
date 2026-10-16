//! Independent, fail-closed TASK-108 closure of TASK-70's six original criteria.
//!
//! This checks the signed evidence *across* the backlog, canonical manifest,
//! frozen oracle matrix and compiling test owners. It does not substitute a
//! registry check for semantic execution: the named public API and independent
//! DuckDB tests run as separate cargo test targets in the same CI job.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

const CLOSURE: &str = include_str!("../docs/task-108-acceptance-freeze.json");
const UPSTREAM: &str = include_str!("../docs/task-107-acceptance-freeze.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const PARENT: &str = include_str!(
    "../.backlog/tasks/task-70 - solve-expression-and-predicate-variants.md"
);
const CHILD: &str = include_str!(
    "../.backlog/tasks/task-108 - signoff-task70-typed-predicate-expression-outcomes.md"
);

fn parse(json: &str) -> Value {
    serde_json::from_str(json).expect("the reviewed acceptance evidence is valid JSON")
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key}: required array"))
}

fn name<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key}: required string"))
}

fn source(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("{path}: missing tracked evidence: {error}"))
}

fn criteria(document: &str) -> Vec<&str> {
    document
        .split_once("<!-- AC:BEGIN -->")
        .expect("acceptance start marker")
        .1
        .split_once("<!-- AC:END -->")
        .expect("acceptance end marker")
        .0
        .lines()
        .filter(|line| line.starts_with("- ["))
        .collect()
}

fn check_named_test(reference: &str) {
    let (file, function) = reference
        .split_once("::")
        .unwrap_or_else(|| panic!("{reference}: expected test_file::test_name"));
    let content = source(file);
    assert!(
        content.contains(&format!("fn {function}(")),
        "{reference}: test owner missing or renamed"
    );
}

#[test]
fn parent_criteria_are_independently_mapped_to_every_merged_child() {
    let closure = parse(CLOSURE);
    let manifest = parse(MANIFEST);
    assert_eq!(closure["task_id"], "TASK-108");
    let children = array(&closure, "upstream_children");
    assert_eq!(children.len(), 7);

    let expected: BTreeSet<_> = (101..=107).map(|task| format!("TASK-{task}")).collect();
    let mut seen = BTreeSet::new();
    for child in children {
        let id = name(child, "task");
        assert!(seen.insert(id.to_owned()), "{id}: duplicate child");
        assert_eq!(name(child, "expected_state"), "Done");
        assert!(name(child, "merged_pr").ends_with(&format!(
            "/{}",
            id.trim_start_matches("TASK-").parse::<usize>().expect("child id") + 8
        )));
        assert_eq!(name(child, "merge_commit").len(), 40);
        let text = source(name(child, "task_file"));
        assert!(
            text.lines().any(|line| line == "status: Done"),
            "{id}: child is not signed off"
        );
        let acs = criteria(&text);
        assert!(!acs.is_empty(), "{id}: no acceptance criteria");
        assert!(
            acs.iter().all(|ac| ac.starts_with("- [x] #")),
            "{id}: an upstream AC is not signed"
        );
    }
    assert_eq!(seen, expected, "TASK-101..107 closure must be exhaustive");
    assert_eq!(
        manifest["task_70_predicate_evidence"]["child_execution_queue"]
            .as_array()
            .expect("canonical ordered queue")
            .len(),
        8
    );

    let original = criteria(PARENT);
    assert_eq!(original.len(), 6, "TASK-70 original AC cannot be rewritten");
    let mappings = array(&closure, "original_ac_map");
    assert_eq!(mappings.len(), 6);
    let expected_families: BTreeSet<_> = array(
        &manifest["task_70_predicate_evidence"],
        "family_evidence",
    )
    .iter()
    .map(|family| name(family, "family"))
    .collect();
    let mut covered_families = BTreeSet::new();
    for (index, mapping) in mappings.iter().enumerate() {
        assert_eq!(
            name(mapping, "ac"),
            format!("AC{}", index + 1),
            "all six original criteria must be mapped in order"
        );
        assert!(original[index].contains(&format!("#{} ", index + 1)));
        for category in ["requires", "modules", "tests", "adapters", "docs"] {
            assert!(
                !array(mapping, category).is_empty(),
                "AC{}: no {category} evidence",
                index + 1
            );
        }
        for path in ["modules", "tests", "adapters", "docs"] {
            for file in array(mapping, path) {
                let file = file.as_str().expect("tracked source path");
                let content = source(file);
                assert!(!content.is_empty(), "AC{}: empty {file}", index + 1);
            }
        }
        for family in array(mapping, "family_ids") {
            let family = family.as_str().expect("family ID");
            assert!(
                expected_families.contains(family),
                "AC{}: invented family {family}",
                index + 1
            );
            covered_families.insert(family);
        }
    }
    assert_eq!(
        covered_families, expected_families,
        "unowned original TASK-70 expression family"
    );
}

#[test]
fn all_frozen_variants_and_dialects_have_executable_two_direction_owners() {
    let closure = parse(CLOSURE);
    let upstream = parse(UPSTREAM);
    let manifest = parse(MANIFEST);
    assert_eq!(closure["frozen_predicate_variant_count"], 42);
    assert_eq!(closure["frozen_dialect_cell_count"], 546);
    assert_eq!(closure["frozen_family_count"], 14);

    let dialects: Vec<_> = array(&closure, "dialects")
        .iter()
        .map(|dialect| dialect.as_str().expect("dialect name"))
        .collect();
    assert_eq!(dialects.len(), 13);
    assert_eq!(closure["dialects"], upstream["frozen_dialects"]);
    let frozen = array(&closure, "predicate_variant_cells");
    let originals = array(&upstream, "inventory_cells");
    assert_eq!(frozen.len(), originals.len());
    let mut count = 0;
    let mut mandatory = 0;
    let mut conditional = 0;
    for (cell, original) in frozen.iter().zip(originals) {
        let id = name(cell, "id");
        assert_eq!(id, format!("E108-{}", name(original, "id")));
        for field in ["feature_id", "variant", "classification", "owner"] {
            assert_eq!(cell[field], original[field], "{id}: contract drift");
        }
        let proofs = array(cell, "truth_proof_tests");
        assert!(!proofs.is_empty(), "{id}: missing independently owned proof");
        for proof in proofs {
            check_named_test(proof.as_str().expect("typed proof name"));
        }

        match name(cell, "classification") {
            "exact_required" => {
                mandatory += 1;
                assert_eq!(
                    original["semantic_exactness_evidence"],
                    "verified_exact_or_impossible",
                    "{id}: unverified mandatory TRUE/NOT TRUE proof"
                );
            }
            "conditional" => {
                conditional += 1;
                assert_eq!(
                    original["conditional_semantic_evidence"],
                    "reviewed_residual_or_exact_under_guards",
                    "{id}: unreviewed residual boundary"
                );
                assert_eq!(
                    cell["conditional_disposition"],
                    "reviewed_residual_or_guarded_exactness_deferred_to_TASK88",
                    "{id}: conditional disposition must remain owned by TASK-88"
                );
            }
            other => panic!("{id}: undocumented classification {other}"),
        }

        let expected = original["dialect_cells"]
            .as_object()
            .expect("upstream per-dialect cases");
        let ids = array(cell, "dialect_fixture_ids");
        assert_eq!(ids.len(), dialects.len());
        for (dialect, evidence_id) in dialects.iter().zip(ids) {
            let status = &expected[*dialect];
            assert_eq!(
                evidence_id.as_str(),
                Some(name(status, "fixture_id")),
                "{id}: dialect evidence changed"
            );
            assert_ne!(name(status, "parser"), "unverified");
            assert_ne!(name(status, "canonical"), "unverified");
            assert_eq!(name(status, "vendor_execution"), if *dialect == "duckdb" {
                "duckdb_oracle_required_where_executable"
            } else {
                "not_claimed"
            });
            count += 1;
        }
    }
    assert_eq!((mandatory, conditional, count), (28, 14, 546));
    let expected: BTreeSet<_> = array(&manifest["task_70_predicate_evidence"], "family_evidence")
        .iter()
        .map(|item| name(item, "family"))
        .collect();
    let mut actual = BTreeSet::new();
    for cell in array(&closure, "family_cells") {
        let family = name(cell, "family");
        assert!(actual.insert(family), "{family}: duplicate family");
        assert!(name(cell, "owner").starts_with("TASK-"));
        assert!(!name(cell, "certified_boundary").is_empty());
        assert!(!name(cell, "evidence").is_empty());
        assert_eq!(cell["release_approved"], false);
    }
    assert_eq!(actual, expected, "TASK-70 family mapping drift");
}

#[test]
fn independently_executed_goldens_seeds_and_adapter_gates_cannot_be_omitted() {
    let closure = parse(CLOSURE);
    let upstream = parse(UPSTREAM);
    for (actual_key, original_key, expected_count) in [
        ("frozen_golden_ids", "golden_cases", 9),
        ("frozen_composition_ids", "composition_cases", 9),
    ] {
        let ids = array(&closure, actual_key);
        let cases = array(&upstream, original_key);
        assert_eq!(ids.len(), expected_count);
        assert_eq!(cases.len(), expected_count);
        for (id, evidence) in ids.iter().zip(cases) {
            assert_eq!(id, &evidence["id"]);
            assert_eq!(evidence["status"], "verified");
        }
    }
    assert_eq!(
        closure["seeded_differential_oracles"],
        upstream["seeds"],
        "independent DuckDB seeds drifted"
    );
    assert_eq!(
        closure["seeded_differential_oracles"],
        serde_json::json!([0, 1, 42, 20261016])
    );
    assert_eq!(
        closure["upstream_final_ci"]["conclusion"],
        "success",
        "TASK-107 merged without a verified CI gate"
    );
    for reference in array(&closure, "independently_owned_tests") {
        check_named_test(reference.as_str().expect("public test owner"));
    }
    let gates = array(&closure, "required_final_gates");
    assert_eq!(gates.len(), 9);
    let unique: BTreeSet<_> = gates
        .iter()
        .map(|gate| gate.as_str().expect("CI gate name"))
        .collect();
    assert_eq!(unique.len(), 9);
    for path in [
        "schema/protocol.schema.json",
        "docs/protocol.md",
        "docs/task-107-conditional-review.md",
    ] {
        assert!(!source(path).is_empty(), "missing canonical {path}");
    }
}

#[test]
fn an_unverified_parent_can_never_be_marked_done_or_release_approved() {
    let closure = parse(CLOSURE);
    let manifest = parse(MANIFEST);
    let signed = &closure["signoff"];
    let parent_done = PARENT.lines().any(|line| line == "status: Done");
    let child_done = CHILD.lines().any(|line| line == "status: Done");
    let parent_criteria = criteria(PARENT);
    let child_criteria = criteria(CHILD);
    assert_eq!(parent_criteria.len(), 6);
    assert_eq!(child_criteria.len(), 5);
    assert_eq!(signed["no_vendor_runtime_claim"], true);
    assert_eq!(signed["downstream_release_gate"], "TASK-91");
    assert_eq!(manifest["task_108_parent_signoff"]["release_approved"], false);
    assert_eq!(manifest["task_107_oracle_evidence"]["release_approved"], false);
    if signed["status"] == "verified" {
        assert_eq!(signed["all_required_checks_green"], true);
        assert_eq!(signed["task_108_checked"], true);
        assert_eq!(signed["task_70_checked"], true);
        assert_eq!(signed["final_head_sha"].as_str().map(str::len), Some(40));
        assert!(child_done && parent_done);
        assert!(parent_criteria.iter().all(|line| line.starts_with("- [x]")));
        assert!(child_criteria.iter().all(|line| line.starts_with("- [x]")));
        assert_eq!(manifest["task_108_parent_signoff"]["status"], "verified");
        assert_eq!(manifest["task_70_predicate_evidence"]["parent_status"], "Done");
    } else {
        assert_eq!(signed["status"], "pending_final_ci");
        assert!(!child_done && !parent_done, "premature parent completion");
        assert!(
            child_criteria.iter().all(|line| line.starts_with("- [ ]")),
            "unchecked TASK-108 requires all gates"
        );
        assert!(
            parent_criteria.iter().all(|line| line.starts_with("- [ ]")),
            "unchecked TASK-70 requires independent closure"
        );
        assert_eq!(manifest["task_108_parent_signoff"]["status"], "pending_final_ci");
    }
}
