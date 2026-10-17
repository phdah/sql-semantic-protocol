---
id: TASK-179
title: Classify conditional session and vendor execution laws
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-178
references:
  - TASK-88
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-88 child 3/4.** Classify conditional session and vendor execution laws. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-88 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `88-C179-P`, `88-C179-N`.
- **Finite variants:** NULL ordering; collations; implicit coercion; FLOAT/timezone; DML/transaction; name case.
- **Independent oracle/verification baseline:** Every vendor-specific law has explicit version/session/evidence, typed residual or real executed oracle.
- **Conditional/external evidence boundary:** No claim of engine execution from DuckDB-equivalent syntax or unsupported parser dialect. Owner TASK-179; release disposition pending review.
- **Original parent AC:** #3, #4, #5, #8, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-88 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

Next: [TASK-180](task-180%20-%20independently-certify-complete-dialect-matrix-and-gate-ci.md), only after completion and merge.
