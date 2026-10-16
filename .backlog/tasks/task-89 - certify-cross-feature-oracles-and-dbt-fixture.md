---
id: TASK-89
title: Differentially certify integrated transformation pipelines
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-77
  - TASK-78
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
  - TASK-84
  - TASK-85
  - TASK-86
  - TASK-87
  - TASK-88
references: 
  - 'sql-tdg TASK-36'
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Operator-local unit tests are insufficient. Sign-off requires holistic constructive plans that generate a complete physical state and produce exact SQL results.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Add deterministic small-instance and seeded/property differential tests validating positive, rejected, impossible and residual claims against DuckDB (and available dialect engines).
- [ ] #2 Cover combinations: filtered joins -> groups/HAVING -> windows/QUALIFY -> sets, correlated subqueries after joins, repeated/self joins, multi-source OR, target constraints, DML/DDL and ordered scripts.
- [ ] #3 Run paired protocol + sql-tdg integration against the committed m-3 dbt fixture, including source constraints, inferred source counts, all terminal outputs and expected negatives.
- [ ] #4 Persist reproducible SQL, seed, input data, protocol snapshot, output snapshots and reason diagnostics for every counterexample.
- [ ] #5 No false exact result allowed; fail CI on any semantic mismatch, missing outcome coverage or disabled fixture.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
- [ ] #7 The committed sql-tdg dbt fixture `make all` runs dbt models **and** the mandatory scripted DuckDB DML/DDL transition harness, asserting complete before/after physical state, terminal output data, multiplicity, row classifications and randomized negative selection across seeds.
- [ ] #8 Match the full approved feature/dialect inventory using parser/canonical equivalence on all dialects and executable DuckDB results for meaning-preserving rewrites; unsupported vendor-only behavior must be explicitly residual and never represented as an executed oracle.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#8 remain required**. Interpret and verify them through the frozen TASK-89 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: committed m-3 dbt + scripted DuckDB DML/DDL project with source-row snapshots and positive/negative outputs in every terminal, independently produced by protocol plus pinned sql-tdg.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: joins -> groups/HAVING -> window/QUALIFY -> set path and a separate correlated subquery/write path; seeded small-instance and randomized differential checks.
3. **Conditional and non-exact boundary:** Release evidence: every reviewed TASK-66 mandatory feature cell has an executed fixture or owner-approved conditional deferral; persist SQL, seed, inputs, protocol, outputs and counterexample reasons.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#8 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Maintainer decision (2026-10-09)

Approved [unified E2E gate](../../docs/coverage-signoff.md): rebuild/extend dbt model DAG for all feasible transformations and add a **required scripted DML/DDL suite invoked by the same top-level `make all`**, not an unrelated optional job. Include CREATE/ALTER/REPLACE/DROP, INSERT/UPDATE/DELETE/MERGE/conflicts, transactions, snapshots, complete output snapshots and per-terminal randomized negatives. No false universal claims for unsupported opaque external behavior.

## TASK-108 expression parent closure handoff (2026-10-17)

The TASK-108 independent acceptance freeze at `docs/task-108-acceptance-freeze.json` maps all six unchanged TASK-70 AC to 14 family owners, all 42 TASK-107 variant cases, 546 dialect cells, nine goldens, nine compositions and four seeds. `tests/task_108_parent_signoff.rs` guards this traceability and requires distinct semantic/oracle CI owners. This does not upgrade global TASK-88/89/91 release evidence or claim unexecuted vendor-engine semantics. TASK-70 remains In Progress pending final TASK-108 PR-head CI and parent signoff.
