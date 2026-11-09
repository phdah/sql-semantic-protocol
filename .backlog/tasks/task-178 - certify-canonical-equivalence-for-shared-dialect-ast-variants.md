---
id: TASK-178
title: Certify canonical equivalence for shared dialect AST variants
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-09'
labels: []
milestone: m-3
dependencies:
  - TASK-177
references:
  - TASK-88
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-88 child 2/5.** Certify canonical equivalence for shared dialect AST variants. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-88 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `88-C178-P`, `88-C178-N`.
- **Finite variants:** JOIN, sets, predicates, windows, DML, DDL, TOP/FETCH/MINUS variants.
- **Independent oracle/verification baseline:** For meaning-preserving spellings assert identical typed domains, NULL/boundary, lineage, membership, count and effects.
- **Conditional/external evidence boundary:** Strip only provenance metadata; do not weaken comparison to parser AST shapes. Owner TASK-178; release disposition pending review.
- **Original parent AC:** #2, #4, #5, #7, #8, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-88 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

Next: [TASK-179](task-179%20-%20classify-conditional-session-and-vendor-execution-laws.md), only after completion and merge.

## Initial implementation PR (2026-11-09)

- **Versioned v1 task-local acceptance baseline:** [docs/task-178-acceptance-freeze.json](../../docs/task-178-acceptance-freeze.json), linked from the coverage manifest. This freezes seven first-party, all-13-dialect representative SQL families (predicates, joins, sets, windows, pagination, INSERT and CTAS) with explicit physical NULL/duplicate source and terminal bags, positive and rejected directions where applicable, and a conservative unsigned matrix.
- **Executable public/native tests:** `tests/task_178_portable_equivalence.rs` checks the exact original TASK-177 sample SQL against complete emitted public protocol documents, and independent DuckDB positive/negative bag oracles, including an inclusive typed predicate lower bound and source-preserving writes.
- **Still blocking:** the complete TASK-178 family/SQL synonym matrix (especially TOP, FETCH, MINUS), 23 preexisting public drift cells, strong typed outcome/oracle coverage for all mandatory cells, native engine/session evidence, direct/dbt/ODCS equivalence and independent review. No new type or semantic support is claimed for unsupported SQL or unexecuted vendors.
- **No AC signed off yet:** the finite task freeze is only a starter subset, not the full AC #1 inventory. TASK-178 remains In Progress and cannot be marked Done or unblock TASK-179 before independent final verification, successful final-head CI, and merge. TASK-88 and TASK-91 remain open.
