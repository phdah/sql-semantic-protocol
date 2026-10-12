//! Guard bounded TASK-68 closure without conflating it with 3.0.0 release approval.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const AUDIT: &str = include_str!("../docs/task-68-signoff.md");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");
const SCHEMA: &str = include_str!("../schema/protocol.schema.json");
const SUITE: &str = include_str!("fixtures/conformance/v1/suite.json");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn task(id: usize) -> String {
    let directory = root().join(".backlog/tasks");
    let prefix = format!("task-{id} - ");
    let matches: Vec<_> = fs::read_dir(&directory)
        .expect("backlog task directory")
        .map(|entry| entry.expect("task entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".md"))
        })
        .collect();
    assert_eq!(matches.len(), 1, "expected one task file for TASK-{id}");
    fs::read_to_string(&matches[0]).expect("read task")
}

fn criteria(text: &str) -> Vec<&str> {
    let section = text
        .split_once("<!-- AC:BEGIN -->")
        .expect("acceptance criteria start")
        .1
        .split_once("<!-- AC:END -->")
        .expect("acceptance criteria end")
        .0;
    section
        .lines()
        .filter(|line| line.starts_with("- ["))
        .collect()
}

#[test]
fn child_queue_is_complete_but_release_gate_is_independent() {
    for id in 93..=100 {
        let text = task(id);
        assert!(
            text.lines().any(|line| line == "status: Done"),
            "TASK-{id} not completed"
        );
        let acs = criteria(&text);
        assert!(!acs.is_empty(), "TASK-{id} has no acceptance evidence");
        assert!(
            acs.iter().all(|line| line.starts_with("- [x] #")),
            "TASK-{id} has an unchecked acceptance criterion"
        );
    }
    let parent = task(68);
    assert!(parent.lines().any(|line| line == "status: Done"));
    let acs = criteria(&parent);
    assert_eq!(acs.len(), 6);
    assert!(acs.iter().all(|line| line.starts_with("- [x] #")));

    let release = task(91);
    assert!(
        !release.lines().any(|line| line == "status: Done"),
        "physical proof closure must not approve the separate release"
    );
    assert!(
        criteria(&release)
            .iter()
            .any(|line| line.starts_with("- [ ] #")),
        "release gate cannot silently become accepted"
    );
    assert!(AUDIT.contains("Release Please PR #79 remain blocked"));
}

#[test]
fn physical_contract_signoff_has_traceable_code_tests_and_json_schema() {
    let manifest: Value = serde_json::from_str(MANIFEST).expect("manifest JSON");
    let evidence = &manifest["task_68_physical_source_evidence"];
    assert_eq!(
        evidence["status"],
        "verified_bounded_physical_source_contract_release_blocked"
    );
    assert_eq!(evidence["owner"], "TASK-68");
    assert_eq!(evidence["signoff_task"], "TASK-100");
    assert_eq!(evidence["signoff_document"], "docs/task-68-signoff.md");
    assert_eq!(evidence["release_gate"], "blocked_TASK_91");
    assert_eq!(
        manifest["review_status"],
        "maintainer_scope_approved_coverage_evidence_pending"
    );

    let expected_children: BTreeSet<_> = (93..=100).map(|id| format!("TASK-{id}")).collect();
    let children: BTreeSet<_> = evidence["child_tasks"]
        .as_array()
        .expect("child list")
        .iter()
        .map(|id| id.as_str().expect("task ID").to_string())
        .collect();
    assert_eq!(children, expected_children);

    for path in evidence["evidence"]
        .as_array()
        .expect("evidence paths")
        .iter()
        .map(|value| value.as_str().expect("evidence path"))
    {
        assert!(
            root().join(path).is_file(),
            "missing physical evidence: {path}"
        );
    }

    let contract: Value = serde_json::from_str(SCHEMA).expect("active protocol schema");
    let graph = &contract["$defs"]["graph"]["properties"];
    for name in [
        "physical_nodes",
        "physical_source_plans",
        "physical_joint_count_plan",
    ] {
        assert!(graph.get(name).is_some(), "missing graph contract: {name}");
    }
    for id in 1..=6 {
        assert!(
            AUDIT.contains(&format!("**#{id}**")),
            "original TASK-68 AC #{id} lacks signoff mapping"
        );
    }
}

#[test]
fn oracle_suite_and_release_limitations_remain_explicit() {
    let suite: Value = serde_json::from_str(SUITE).expect("conformance suite");
    assert_eq!(suite["owner"], "phdah/sql-semantic-protocol");
    let raw = suite["raw_sql"].as_array().expect("raw SQL fixtures");
    assert_eq!(raw.len(), 7);
    for fixture in raw {
        let path = fixture["path"].as_str().expect("SQL path");
        assert!(
            root()
                .join("tests/fixtures/conformance/v1")
                .join(path)
                .is_file(),
            "missing SQL conformance fixture {path}"
        );
    }
    assert_eq!(
        suite["dbt"]["models"]
            .as_array()
            .expect("dbt model inventory")
            .len(),
        11
    );

    let manifest: Value = serde_json::from_str(MANIFEST).expect("manifest JSON");
    let evidence = &manifest["task_68_physical_source_evidence"];
    let unproved = evidence["unproved_scope"]
        .as_array()
        .expect("fail-closed scope");
    assert!(!unproved.is_empty());
    for case in [
        "unproved_general_multi_join_operator_chain",
        "unproved_general_non_bijective_scalar_inversion",
    ] {
        assert!(
            unproved.iter().any(|item| item == case),
            "must retain residual coverage: {case}"
        );
    }
    assert_eq!(
        manifest["task_99_protocol_conformance_contract"]["protocol_release_gate"],
        "blocked_TASK_91"
    );
    assert_eq!(
        manifest["task_99_protocol_conformance_contract"]["sql_tdg_downstream_generator_signoff"],
        "pending_TASK_31_35_36"
    );
    for evidence_path in [
        "tests/protocol_conformance_physical.rs",
        "tests/protocol_conformance_dialect_laws.rs",
        "tests/protocol_conformance_dbt_e2e.rs",
        "tests/physical_write_states.rs",
        "tests/physical_write_adapter_parity.rs",
    ] {
        assert!(Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(evidence_path)
            .is_file());
    }
}
