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
  - TASK-181
  - TASK-182
  - TASK-183
  - TASK-184
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

## TASK-108 completed handoff (2026-10-17)

TASK-70/108 closed nine expression-specific closed-world DuckDB goldens, nine compositions, four seeds and direct SQL/dbt/ODCS parity. TASK-89's mixed-feature and scripted DML/DDL/dbt full-pipeline acceptance remains unchecked; this local closure does not certify it.

## Bounded dependency-ordered child execution queue (2026-10-17)

TASK-89 is an **independent parent signoff** and cannot be closed from a narrow passing CI subset. All original AC remain mandatory and unchecked until every child below is Done and merged. Choose first unfinished child with satisfied upstream dependencies; no child depends on TASK-89. `docs/backlog-child-queue-plan.json` is the versioned planning inventory, and each child must register a frozen, executable task-specific acceptance matrix before semantics change.

- **[TASK-181](task-181%20-%20build-seeded-differential-harness-with-replayable-evidence.md)** (1/4): deterministic small instances; positive/negative/Impossible/residual; counterexample capture. Fixtures `89-C181-P`, `89-C181-N`.
- **[TASK-182](task-182%20-%20certify-mixed-relational-and-correlated-full-output-paths.md)** (2/4): filtered JOIN->GROUP/HAVING->QUALIFY->set; correlated subquery over join; shared keys. Fixtures `89-C182-P`, `89-C182-N`.
- **[TASK-183](task-183%20-%20certify-pinned-sql-tdg-dbt-and-dml-ddl-execution-harness.md)** (3/4): sql-tdg pinned SHA; committed m-3 fixture; make all; actual scripted transitions. Fixtures `89-C183-P`, `89-C183-N`.
- **[TASK-184](task-184%20-%20independently-sign-off-cross-feature-and-dialect-release-matrix.md)** (4/4): TASK-66 variants; 13 dialect equivalence; approved conditional engine laws. Fixtures `89-C184-P`, `89-C184-N`.

**Original AC ownership:** #1 -> TASK-181, TASK-182, TASK-184; #2 -> TASK-182, TASK-184; #3 -> TASK-183, TASK-184; #4 -> TASK-181, TASK-182, TASK-183, TASK-184; #5 -> TASK-181, TASK-182, TASK-183, TASK-184; #6 -> TASK-181, TASK-182, TASK-183, TASK-184; #7 -> TASK-183, TASK-184; #8 -> TASK-182, TASK-184.

**No premature Done:** mandatory typed exactness cannot be deferred as residual; unverified cells block completion. Positive/NOT TRUE rows, nullable interval output domains, full DuckDB bags, adapter/13-dialect evidence, reviewed conditional vendor cases and final CI are necessary. TASK-91 remains a distinct release decision after TASK-89 closure.
