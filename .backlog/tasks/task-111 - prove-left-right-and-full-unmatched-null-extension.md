---
id: TASK-111
title: Prove LEFT RIGHT and FULL unmatched null-extension
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-110
references:
  - TASK-71
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-71 child 3/8.** Prove LEFT RIGHT and FULL unmatched null-extension. Work in a separate focused PR against `main` after its listed dependencies are Done and merged. The original TASK-71 acceptance criteria remain mandatory for parent signoff. This child is independent of its parent as a dependency; it must not depend on TASK-71.
<!-- SECTION:DESCRIPTION:END -->

## Frozen planning scope

- **Stable acceptance fixture IDs:** `71-C111-P`, `71-C111-N`. Register them as immutable, executable versioned fixtures in the task-specific coverage matrix **before changing semantics**.
- **Named feature/variant inventory (finite):** LEFT/RIGHT/FULL matched and unmatched; SQL NULL and reversed-ON operands; physical vs synthetic NULL.
- **Concrete independent oracle baseline:** L keys [1,NULL,2], R empty => LEFT/FULL=3 rows; reversed operands preserve LEFT count=3; R-only RIGHT count=3.
- **Conditional/fail-closed boundary:** Missing partner is an entire-physical-bag absence proof, never a physical NULL insertion.
- **Original parent AC ownership:** #1, #3, #5, #6. Additive to project-wide outcome-first definition of done and TASK-66 capability inventory.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Freeze the two named fixture IDs and **every named variant above** into this child's versioned executable matrix: exact SQL/AST/dialect classification, source bag, stable source/column instance IDs, mandatory outcome domains (NULL and inclusive/exclusive bounds), positive/NOT TRUE proof, expected full output bag, adapter facts and explicitly owned conditional residuals; link the matrix from `docs/coverage-manifest.json`. Missing cells block Done.
- [x] #2 Implement the **finite named variants** in the canonical semantic/physical model. Match the stated independent oracle baseline exactly, including multiplicity, value-domain constraints and correct Exact/Impossible/Residual directions. No consumer SQL reparsing, invented physical rows, unknown-as-exact claims or mandatory variants silently relegated to Residual.
- [x] #3 Execute the baseline and a complementary rejected/NULL/duplicate or impossible case using complete DuckDB physical input and terminal-output bags. Assert individual row identities, counts, domain bounds and closed-world absence where applicable, not only a successful parser or a row count.
- [x] #4 For each supported shared form, classify all **13 exposed project dialects** as parser-equivalent or explicit parser boundary; check direct SQL and dbt plus ODCS when equivalent metadata exists. For each conditional vendor/session law, record specific missing evidence, stable typed residual/Impossible, owner TASK-111 and pending reviewed release disposition. No native engine certification without execution.
- [x] #5 Run focused public API tests, `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and active protocol/schema snapshots on the final implementation head. Review actual assertions against each frozen fixture; only then mark this child Done and update its parent queue and coverage evidence. Green CI alone is insufficient.
<!-- AC:END -->

## Handoff

Next child: [TASK-112](task-112%20-%20certify-semi-and-anti-absence-witnesses.md). Do not begin the next child until this task is Done **and its PR merged**. Do not mark TASK-71 Done just because one child is complete.

## Bounded TASK-111 signoff

- Frozen v2 matrix: [TASK-111 acceptance freeze](../../docs/task-111-acceptance-freeze.json) (8 mandatory cases, 104 canonical parser dialect cells).
- Independent evidence and two-axis code review: [TASK-111 acceptance audit](../../docs/task-111-acceptance-audit.md).
- Public tests: `tests/task_111_outer_join.rs`, `tests/task_111_outer_join_adapters.rs`, and `tests/task_111_acceptance_freeze.rs`.
- Nine implementation-head CI jobs passed: https://github.com/phdah/sql-semantic-protocol/actions/runs/39041937842. Merge additionally requires the final PR metadata head checks to pass.
- Delivery: https://github.com/phdah/sql-semantic-protocol/pull/120.
- Reviewed conditional boundaries: physical SQL NULL insertion requires authoritative source metadata; native vendor/session laws and nonportable NULL-safe equality remain typed residual and release-pending, not mandatory portable-equality claims.
- TASK-71 stays In Progress. TASK-112 may start only after this PR is merged to `main`.
