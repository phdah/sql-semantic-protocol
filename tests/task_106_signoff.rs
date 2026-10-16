//! Regression guard for TASK-106's explicitly finite acceptance contract.
//!
//! Semantic correctness belongs to the public-API and DuckDB oracle tests named
//! by the task. This test prevents accidentally widening, losing, or signing
//! off an unverified acceptance matrix.

use std::collections::BTreeSet;
use std::path::Path;

const TASK: &str =
    include_str!("../.backlog/tasks/task-106 - compose-computed-predicates-across-relational-graphs.md");
const TESTS: &str = include_str!("task_106_composed_predicates.rs");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");

#[test]
fn bounded_task_106_has_a_frozen_complete_evidence_matrix() {
    let expected: BTreeSet<&str> = [
        "F1", "F2", "F3", "J1", "J2", "G1", "G2", "R1", "R2", "JG1",
        "W1", "X1", "A1", "A2", "A3",
    ]
    .into_iter()
    .collect();

    let rows: Vec<_> = TASK
        .split_once("## Frozen TASK-106 acceptance scope")
        .expect("frozen TASK-106 scope")
        .1
        .split_once("## Implementation Plan")
        .expect("scope ends before implementation history")
        .0
        .lines()
        .filter(|line| line.starts_with("| "))
        .filter_map(|line| {
            let mut columns = line.split('|').skip(1).map(str::trim);
            let id = columns.next()?;
            if id == "ID" || id.starts_with("---") {
                return None;
            }
            let required = columns.next()?;
            let expected_outcome = columns.next()?;
            let evidence = columns.next()?;
            assert!(!required.is_empty(), "{id}: missing certified rule");
            assert!(!expected_outcome.is_empty(), "{id}: missing outcome class");
            assert!(!evidence.is_empty(), "{id}: missing evidence");
            Some((id, evidence))
        })
        .collect();

    let actual: BTreeSet<_> = rows.iter().map(|(id, _)| *id).collect();
    assert_eq!(actual, expected, "TASK-106 proof coverage scope drift");
    assert_eq!(rows.len(), expected.len(), "duplicate matrix IDs");

    for (id, evidence) in rows {
        let references: Vec<_> = evidence
            .split('`')
            .enumerate()
            .filter_map(|(index, part)| (index % 2 == 1).then_some(part))
            .collect();
        assert!(!references.is_empty(), "{id}: missing cited regression");
        for reference in references {
            if reference.starts_with("tests/") || reference.starts_with("docs/") {
                assert!(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join(reference)
                        .is_file(),
                    "{id}: missing evidence path {reference}"
                );
            } else {
                assert!(
                    TESTS.contains(&format!("fn {reference}(")),
                    "{id}: missing public-API regression {reference}"
                );
            }
        }
    }
}

#[test]
fn task_106_signoff_requires_all_five_acceptance_criteria_but_not_release_approval() {
    assert!(
        TASK.lines().any(|line| line == "status: Done"),
        "TASK-106 must not be signed off before evidence verification"
    );
    let acceptance = TASK
        .split_once("<!-- AC:BEGIN -->")
        .expect("AC start")
        .1
        .split_once("<!-- AC:END -->")
        .expect("AC end")
        .0;
    let criteria: Vec<_> = acceptance
        .lines()
        .filter(|line| line.starts_with("- ["))
        .collect();
    assert_eq!(criteria.len(), 5);
    for (index, criterion) in criteria.iter().enumerate() {
        assert!(
            criterion.starts_with(&format!("- [x] #{} ", index + 1)),
            "TASK-106 AC #{} is not signed off",
            index + 1
        );
    }

    let manifest: serde_json::Value =
        serde_json::from_str(MANIFEST).expect("valid coverage manifest");
    assert!(
        manifest.to_string().contains("TASK-106"),
        "canonical coverage must still identify its owning task"
    );
    let release = include_str!("../.backlog/tasks/task-91 - gate-single-protocol-3-release-on-tdg-signoff.md");
    assert!(
        !release.lines().any(|line| line == "status: Done"),
        "TASK-106 sign-off must not silently approve TASK-91"
    );
}
