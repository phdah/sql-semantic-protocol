---
id: TASK-80
title: Prove INSERT and overwrite/source append semantics
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-79
  - TASK-145
  - TASK-146
  - TASK-147
  - TASK-148
references: 
  - 'TASK-65'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Simple INSERT SELECT does not cover explicit/missing columns, VALUES, INSERT OVERWRITE or dialect conflict actions.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize INSERT VALUES, INSERT SELECT, DEFAULT VALUES, column lists, omitted/default/generated columns, multi-row, INSERT OVERWRITE and supported dialect INSERT IGNORE/REPLACE/ON CONFLICT/UPSERT/RETURNING forms.
- [ ] #2 Represent target prestate, source query, column bindings, cast/default evaluations, conflict resolution, overwrite scope, key checks and final-state bag effects.
- [ ] #3 Prove matching/rejected effects and exact touched/untouched rows and output cardinality where supported; expose ambiguity/engine-specific conflict order as residual.
- [ ] #4 Compose writes with preceding CTEs and subsequent reads/DDL; verify repeated application and idempotence only when demonstrable.
- [ ] #5 Test full pre/post DuckDB snapshots and per-dialect parse/engine forms with NULL, duplicates, constraints and zero-row insertions.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-80 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: INSERT VALUES multirow, DEFAULT VALUES, INSERT SELECT, column reorder/omission, defaults and generated columns with exact initial/final bags.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: INSERT following CTE with constraints and subsequent SELECT, plus conflict/overwrite/RETURNING on a deterministic portable shape.
3. **Conditional and non-exact boundary:** Inventory conditional: IGNORE/REPLACE/UPSERT/ON CONFLICT, OVERWRITE scope and vendor conflict resolution; classify nonportable multiple-match/error semantics.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Bounded dependency-ordered child implementation queue (2026-10-17)

TASK-80 is a **parent signoff**, not a single implementation PR. Preserve all original AC and only sign them off independently after all children are Done and merged. Pick the first unfinished dependency-ready child. Parent depends on the children only for closure; no child depends on its parent. Scope and fixed fixture IDs are in `docs/backlog-child-queue-plan.json`.

- **[TASK-145](task-145%20-%20construct-typed-insert-values-and-default-values-state.md)** (1/4): multirow VALUES; explicit lists; omitted/default/generated/identity fields; zero-row insert. IDs `80-C145-P`, `80-C145-N`.
- **[TASK-146](task-146%20-%20compose-insert-select-column-reorder-and-cte-sources.md)** (2/4): INSERT SELECT; CTE filtered source; target reordering; FK and CHECK constraints. IDs `80-C146-P`, `80-C146-N`.
- **[TASK-147](task-147%20-%20classify-conflict-overwrite-returning-and-upsert-syntax.md)** (3/4): INSERT IGNORE/REPLACE/ON CONFLICT; OVERWRITE; RETURNING; vendor dialects. IDs `80-C147-P`, `80-C147-N`.
- **[TASK-148](task-148%20-%20certify-ordered-insert-scripts-and-adapter-parity.md)** (4/4): INSERT after CTE; later SELECT/DDL; direct/dbt/ODCS; 13 dialect parse matrix. IDs `80-C148-P`, `80-C148-N`.

**Original AC mapping:** #1 -> TASK-145, TASK-146, TASK-147, TASK-148; #2 -> TASK-145, TASK-146, TASK-147, TASK-148; #3 -> TASK-145, TASK-146, TASK-147, TASK-148; #4 -> TASK-146, TASK-148; #5 -> TASK-145, TASK-146, TASK-147, TASK-148; #6 -> TASK-145, TASK-146, TASK-147, TASK-148.

**No premature completion:** Each child freezes and verifies its own exact expected source/target data, output domains, dual-direction witnesses, parser/dialect/adapter cells, and final CI; the parent gets separate whole-AC closure. A conditional vendor limitation must name an owner and approved evidence boundary; it cannot hide a mandatory unimplemented family.
