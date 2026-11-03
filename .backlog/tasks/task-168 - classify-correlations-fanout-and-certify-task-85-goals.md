---
id: TASK-168
title: Classify correlations fanout and certify TASK-85 goals
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-03'
labels: []
milestone: m-3
dependencies:
  - TASK-167
references:
  - TASK-85
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-85 child 4/4.** Classify correlations fanout and certify TASK-85 goals. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-85 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `85-C168-P`, `85-C168-N`.
- **Finite variants:** tuple histograms; multicolumn correlation; LIMIT/QUALIFY/window/set fanout.
- **Independent oracle/verification baseline:** Run positive/impossible/residual oracle triplet and paired SQL/dbt outcomes with source/terminal full snapshots.
- **Conditional/external evidence boundary:** Unverified tuple correlations remain typed residual; frozen cells control required exact richness. Owner TASK-168; release disposition pending review.
- **Original parent AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-85 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

After this child, audit and sign off TASK-85 separately; TASK-91 controls release readiness.

## TASK-168 implementation evidence (2026-11-03)

- [Immutable preimplementation v1 freeze](../../docs/task-168-acceptance-freeze-v1.json) and [versioned v2 executed target matrix](../../docs/task-168-acceptance-freeze.json) preserve seven positive, impossible, and residual source/terminal cases across 13 canonical parser dialects (91 classifications); the original v1 lacks some per-cell richness. The v2 correction is **not** described as prospective completeness. Owner disposition of this historical AC #1 discrepancy is required before signoff.
- [Canonical tuple proof](../../src/physical_realization/joint_tuple_goals.rs) checks signed INTEGER/SQL NULL correlated tuple frequencies against one exact physical source assignment, with separate source-complete JOIN/GROUP + WINDOW/SET and LIMIT/QUALIFY/rank interpreters. Cross-row correlation is never inferred by multiplying scalar histograms; contradictions return `Impossible` and unproved text/order/engine behavior returns `Residual`.
- [Independent frozen oracle inventory](../../tests/task_168_frozen_acceptance.rs) checks complete native DuckDB source and terminal bags, all 91 parser cells, selected/NOT TRUE physical occurrences, exact typed NULL and inclusive numeric value domains, and active typed witness emission. [Shared adapter parity](../../tests/task_167_adapter_parity.rs) covers direct SQL, compiled dbt + authoritative warehouse catalog, and ODCS for equivalent source facts, including negative and residual cases. [Single-terminal oracle](../../tests/task_168_tuple_fanout.rs) exercises LIMIT, QUALIFY, duplicates and empty full-bag proofs. 
- [Opt-in sidecar](../../src/emission/tuple_goals.rs) emits only source-complete `Exact` bags; negative and residual verdicts contain no fabricated source or result. The active protocol root and `schema/protocol.schema.json` are unchanged.
- [Pinned downstream sql-tdg qualification](https://github.com/phdah/sql-semantic-protocol/actions/runs/41725446712) passes the selected lib/type/full-query/exactness suites using consumer `3beaf7799ab54b2440ae34c917ff7eda6c11c398`; two known timestamp tests are compared with the fixed baseline and retain the same computed-expression residual. No new vendor-engine execution or consumer release certification is asserted.

**Outstanding signoff:** rerun full standard CI and complete task-specific matrix checks on the final semantic head; ensure the temporary pinned workflow is deleted after qualification. Run Matt code-review Standards + Spec and obtain a genuinely independent reviewer or an explicit task-specific owner waiver. Only then sign AC #1..#5 and change the task to Done. Parent TASK-85 and release TASK-91 remain open.
