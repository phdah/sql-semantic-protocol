---
id: TASK-184
title: Independently sign off cross-feature and dialect release matrix
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-183
references:
  - TASK-89
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-89 child 4/4.** Independently sign off cross-feature and dialect release matrix. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-89 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `89-C184-P`, `89-C184-N`.
- **Finite variants:** TASK-66 variants; 13 dialect equivalence; approved conditional engine laws.
- **Independent oracle/verification baseline:** All mandatory rows executed or evidenced; zero disabled fixtures; no false Exact; full CI green.
- **Conditional/external evidence boundary:** TASK-91 retains separate publication/consumer gate even after TASK-89 closure. Owner TASK-184; release disposition pending review.
- **Original parent AC:** #1, #2, #3, #4, #5, #6, #7, #8.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-89 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

After this child, audit and sign off TASK-89 separately; TASK-91 controls release readiness.
