---
id: TASK-83
title: Model complete relation-producing and destructive DDL effects
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
  - TASK-77
  - TASK-78
  - TASK-79
  - TASK-157
  - TASK-158
  - TASK-159
  - TASK-160
references: 
  - 'TASK-12'
  - 'TASK-23'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CREATE/REPLACE alters identities and state differently from CTAS and VIEW; dropping, renaming, truncating and schema changes break downstream dependencies.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize CREATE [OR REPLACE] TABLE AS, VIEW, MATERIALIZED VIEW, TEMP/TEMPORARY/TRANSIENT TABLE, LIKE/CLONE where supported, and schema-qualified definitions.
- [ ] #2 Represent DROP, TRUNCATE, ALTER TABLE ADD/DROP/RENAME/TYPE columns, RENAME TABLE/VIEW and replacement/overwrite effects on named datasets and constraints.
- [ ] #3 Distinguish logical view definitions, materialized snapshots and physical stored tables, scoping of temporary objects, IF [NOT] EXISTS and dialect-specific DDL transaction rules.
- [ ] #4 Expose exact before/after schema and relation existence, source lineage, data preservation/loss and read dependencies; fail explicitly on unmodeled structural changes.
- [ ] #5 Execute statement sequences and verify relation catalogs plus data snapshots for portable DuckDB cases and dialect-specific variants.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-83 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: CREATE TABLE/CTAS, CREATE OR REPLACE, VIEW vs materialized VIEW, DROP/TRUNCATE and basic ALTER ADD/DROP/RENAME/TYPE with schema+data snapshot.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: CREATE/INSERT/ALTER/SELECT/DROP scripted relation lifecycle, IF EXISTS/NOT EXISTS and lineage to physical sources.
3. **Conditional and non-exact boundary:** Inventory conditional: CLONE/LIKE, temporary/transient scope and vendor DDL transaction laws; missing runtime/engine semantics stays explicit.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Bounded dependency-ordered child implementation queue (2026-10-17)

TASK-83 is a **parent signoff**, not a single implementation PR. Preserve all original AC and only sign them off independently after all children are Done and merged. Pick the first unfinished dependency-ready child. Parent depends on the children only for closure; no child depends on its parent. Scope and fixed fixture IDs are in `docs/backlog-child-queue-plan.json`.

- **[TASK-157](task-157%20-%20prove-create-ctas-view-and-materialization-identities.md)** (1/4): CREATE TABLE, CTAS, REPLACE; VIEW/MATERIALIZED VIEW; schema-qualified IDs. IDs `83-C157-P`, `83-C157-N`.
- **[TASK-158](task-158%20-%20prove-alter-add-drop-rename-type-column-transitions.md)** (2/4): ALTER TABLE ADD/DROP/RENAME/TYPE; defaults; data preservation; key checks. IDs `83-C158-P`, `83-C158-N`.
- **[TASK-159](task-159%20-%20prove-drop-truncate-rename-if-exists-and-vendor-object-kinds.md)** (3/4): DROP, TRUNCATE, RENAME, IF EXISTS/NOT EXISTS, CLONE/LIKE, TEMP/TRANSIENT. IDs `83-C159-P`, `83-C159-N`.
- **[TASK-160](task-160%20-%20compose-create-insert-alter-select-drop-and-certify-task-83.md)** (4/4): full ordered schema+row lifecycle, catalog identities, adapter parity. IDs `83-C160-P`, `83-C160-N`.

**Original AC mapping:** #1 -> TASK-157, TASK-159, TASK-160; #2 -> TASK-158, TASK-159, TASK-160; #3 -> TASK-157, TASK-159, TASK-160; #4 -> TASK-157, TASK-158, TASK-159, TASK-160; #5 -> TASK-157, TASK-158, TASK-159, TASK-160; #6 -> TASK-157, TASK-158, TASK-159, TASK-160.

**No premature completion:** Each child freezes and verifies its own exact expected source/target data, output domains, dual-direction witnesses, parser/dialect/adapter cells, and final CI; the parent gets separate whole-AC closure. A conditional vendor limitation must name an owner and approved evidence boundary; it cannot hide a mandatory unimplemented family.
