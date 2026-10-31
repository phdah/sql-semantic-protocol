---
id: TASK-78
title: Model recursive CTEs and producer/derived-scope exactness
status: Done
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-31'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-76
  - TASK-77
  - TASK-141
  - TASK-142
  - TASK-143
  - TASK-144
references: 
  - 'TASK-31'
  - 'TASK-44'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Named producer layers, alias scopes, WITH RECURSIVE and row-changing CTEs require proofs that survive physical materialization boundaries.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Cover ordinary and nested CTEs, derived tables, correlated LATERAL, views, materialized relations, reuse, shadowing and multi-statement dependency ordering.
- [x] #2 Handle supported recursive CTE base/recursive terms, UNION [ALL], cycle checks and termination bounds when a finite constructive proof exists; otherwise return a typed termination residual.
- [x] #3 Prove upstream/downstream column mapping including computed/renamed columns, fanout, aggregation, filters, limits and repeated physical dependencies.
- [x] #4 Preserve source and produced relation identities and producer read/write semantics; forbid treating an intermediate witness as a free source insertion.
- [x] #5 E2E-test multi-step dbt and raw scripts with selective row-changing intermediate models and reused sources.
- [x] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-78 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: nested/reused/shadowed ordinary CTEs and derived views with stable physical column identity; materialization distinct from inline scope.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: one selectively filtering or computed producer across two consumers, then a downstream join/count or write, proven from original physical inputs.
3. **Conditional and non-exact boundary:** Inventory conditional: WITH RECURSIVE bounded base/step with termination proof; cycle/unbounded recursion residual; LATERAL, temp objects and multi-statement ordering classified.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Dependency-ordered bounded child queue (2026-10-17)

**TASK-78 is now a parent signoff, not one implementation PR.** Its original AC remain unchecked. Every child has its own finite variant/fixture IDs, complete source/terminal oracle and explicit conditional boundaries in `docs/backlog-child-queue-plan.json`. Select the first unfinished dependency-ready child; never add a parent dependency to a child. Do not sign off the parent merely because one PR passes CI.

- **[TASK-141](task-141%20-%20prove-nested-and-shadowed-cte-source-identity.md)** (1/4): ordinary WITH; nested/reused CTEs; alias shadowing; view vs materialized copy. Fixtures `78-C141-P`, `78-C141-N`.
- **[TASK-142](task-142%20-%20classify-finite-recursive-cte-and-cycle-termination-laws.md)** (2/4): recursive base and step; UNION vs UNION ALL; finite bound; cyclic residual. Fixtures `78-C142-P`, `78-C142-N`.
- **[TASK-143](task-143%20-%20compose-filtering-computed-producers-across-consumers.md)** (3/4): CTE WHERE; computed aliases; downstream JOIN/count/write; materialization. Fixtures `78-C143-P`, `78-C143-N`.
- **[TASK-144](task-144%20-%20certify-raw-dbt-multistep-scopes-and-task-78-signoff.md)** (4/4): views, reused sources, multi statements, selective row-changing materialization; dialects. Fixtures `78-C144-P`, `78-C144-N`.

**Original acceptance mapping:** #1 -> TASK-141, TASK-142, TASK-143, TASK-144; #2 -> TASK-142, TASK-144; #3 -> TASK-141, TASK-142, TASK-143, TASK-144; #4 -> TASK-141, TASK-143, TASK-144; #5 -> TASK-141, TASK-142, TASK-143, TASK-144; #6 -> TASK-141, TASK-142, TASK-143, TASK-144.

**Closure gate:** All child PRs merged, every original AC independently verified, every mandatory outcome/execution oracle green, conditional vendor/session cases owner-reviewed, adapter and 13-dialect matrix complete, final CI green. Neither parser coverage nor residual classification alone excuses a required exact variant. TASK-91 remains separately release-blocked.

## Independent parent audit (2026-10-31)

TASK-141..144 are Done and their PRs #167..170 are merged on `main`.
The task-owned independent original-AC v1 crosswalk is
[`docs/task-78-parent-acceptance-freeze.json`](../../docs/task-78-parent-acceptance-freeze.json),
with [audit](../../docs/task-78-parent-acceptance-audit.md) and
[`tests/task_78_parent_closure.rs`](../../tests/task_78_parent_closure.rs).
The crosswalk indexes 20 mandatory child proof cases and 9 reviewed conditional cases.
All original AC #1..#6 were independently checked against the bounded child evidence, and the reviewed implementation head passed all nine CI jobs. TASK-91 remains a separate release gate.

## Final Summary (2026-10-31)

All four TASK-141..144 implementation PRs are Done and merged to main.
The task-owned parent freeze covers 20 mandatory cases and 9 reviewed conditional
Residual boundaries, with complete typed terminal/source evidence and reviewed
13 canonical parser dialects plus the PostgreSQL alias. All six original
parent acceptance criteria have been independently cross-checked, including
source/producer provenance, finite recursion, typed outcome domains,
positive/negative physical proof and SQL/dbt/ODCS evidence where equivalent.

Independent implementation-head CI [run 41242073407](https://github.com/phdah/sql-semantic-protocol/actions/runs/41242073407)
passed all nine checks including Rust tests with/without default features,
strict lint, format, docs and dbt Core E2E. Matt's [two-axis Standards/Spec
review](https://github.com/phdah/sql-semantic-protocol/pull/171#pullrequestreview-5724940824)
found no blocking findings. GitHub review is COMMENTED, not a distinct-account approval.

The signoff metadata commit must itself pass final-head CI before PR #171 is
merged. TASK-91 and unproven cross-engine execution laws remain separate.
