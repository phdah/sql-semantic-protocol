---
id: TASK-165
title: Prove standalone output count and typed distribution goals
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-02'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-79
references:
  - TASK-85
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-85 child 1/4.** Prove standalone output count and typed distribution goals. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-85 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `85-C165-P`, `85-C165-N`.
- **Finite variants:** rows; groups; distinct; NULL fraction; duplicate count; column histogram.
- **Independent oracle/verification baseline:** source [1,1,NULL] => count3, distinct nonnull1, NULL count1, histogram {1:2,NULL:1}.
- **Conditional/external evidence boundary:** Full closed-world bag mandatory before Exact histogram classification. Owner TASK-165; release disposition pending review.
- **Original parent AC:** #1, #3, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [x] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [x] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [x] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-85 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

Next: [TASK-166](task-166%20-%20compose-shared-source-positive-and-impossible-terminal-goals.md), only after completion and merge.

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Working branch: `feat/task-165-standalone-output-distributions`.
- Task-owned bounded matrix: `docs/task-165-acceptance-freeze.json` (active v3; v1/v2 archived without erasing the late-v1-freeze history; v3 source bag matches the actual Groups witness; eight positive/negative/conditional cases and 104 parser-dialect cells).
- Public API proof seam: complete SQL output goals and canonical source-row witnesses. Exact histogram statistics are derived, not duplicated protocol facts; NULL ratio is rational, not floating-point.
- Standalone direct-SQL and DuckDB fixture coverage: `tests/task_165_standalone_goals.rs`.
- Executable frozen matrix: `tests/task_165_acceptance_inventory.rs`; direct/dbt catalog/ODCS parity: `tests/task_165_adapter_parity.rs`; review and signoff accounting: `docs/task-165-acceptance-audit.md`.
- V3 strengthens exact constructive replay: SourceRows [1,1,NULL], grouped physical [0,1] (one row per key), and complete corresponding DuckDB terminal bags across 13 dialect parser paths.
- Pinned sql-tdg regression qualification: https://github.com/phdah/sql-semantic-protocol/actions/runs/41522367446, 69 selected tests passed; two unrelated timestamp tests fail identically on pre-PR base and branch (deferred to TASK-89/91).
- Remaining before child Done: final metadata-head 9/9 CI, independent two-axis review of each mandatory cell, and explicit disposition of original AC #1 timing deviation (v1 freeze followed initial tests). TASK-165 AC #2/#3/#4 have implementation-backed evidence, whereas AC #1 (historical freeze sequencing) and #5 (independent review) remain unchecked. Parent TASK-85 stays open.
<!-- SECTION:NOTES:END -->
