---
id: TASK-167
title: Compose join filter group and another window/set terminal
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-03'
labels: []
milestone: m-3
dependencies:
  - TASK-166
references:
  - TASK-85
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-85 child 3/4.** Compose join filter group and another window/set terminal. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-85 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `85-C167-P`, `85-C167-N`.
- **Finite variants:** join->WHERE->GROUP/HAVING; additional window or set output; correlated tuples.
- **Independent oracle/verification baseline:** L keys [1,1], R key [1] => join count2, grouped key1 count2; second terminal share L count2.
- **Conditional/external evidence boundary:** One closed physical row assignment satisfies all terminal histograms and NULL domains. Owner TASK-167; release disposition pending review.
- **Original parent AC:** #1, #2, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-85 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

Next: [TASK-168](task-168%20-%20classify-correlations-fanout-and-certify-task-85-goals.md), only after completion and merge.

## TASK-167 implementation (2026-11-03)

- Prospective acceptance matrix: [v1](../../docs/task-167-acceptance-freeze.json), linked from the coverage manifest.
- Initial implementation: snapshot-relative joint certification of a two-source INNER JOIN/WHERE/GROUP/HAVING terminal and an independent window or set-tree terminal using the *same* complete physical source bags. Unsupported semantics remain Residual; exact full-bag contradictions are Impossible.
- Public API and full-bag DuckDB tests: `src/physical_realization/joint_terminals.rs` and `tests/task_167_joint_terminals.rs`.
- **Not Done:** CI, independent review, task-owned adapter parity, source-to-protocol serialization, and pinned sql-tdg qualification are not yet independently verified. No acceptance criteria have been signed off. Parent TASK-85 remains open; TASK-168 remains blocked until this child is Done and merged.
