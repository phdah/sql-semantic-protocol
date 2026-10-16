---
id: TASK-87
title: Ensure SQL, dbt, ODCS and schema evidence parity
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-68
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
references: 
  - 'TASK-27'
  - 'TASK-35'
  - 'TASK-57'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
New canonical semantics must flow through every source that can supply equivalent facts and must not regress rich dbt artifacts or warehouse metadata.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Map source of truth per fact: compiled model SQL, dependency graph, declared constraints/tests, catalog warehouse schema, ODCS schema evidence and direct SQL DDL.
- [ ] #2 Implement same canonical typed evidence across adapters where available; report absent inputs or unsupported metadata as explicit diagnostics, not guessed contracts.
- [ ] #3 Cover dbt ephemeral/view/table/incremental materialization evidence without treating SELECT-only manifests as executed stateful writes.
- [ ] #4 Resolve schema-qualified identities, quoted names, model tests, FK-only sources, source/target types and cross-layer cardinality obligations.
- [ ] #5 Add paired SQL/dbt/ODCS equivalence snapshots and adapter-specific inability/residual tests for new fields.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-87 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: authoritative dbt manifest compiled SQL/dependencies/constraints and catalog datatypes, direct SQL DDL and ODCS schema mapped to one canonical fact contract.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: parity for one table model, one view/ephemeral model and one incremental/write scenario with source identity and typed domain checks.
3. **Conditional and non-exact boundary:** Inventory conditional: unavailable warehouse introspection, schema-qualified ambiguous names, unknown test enforcement and FK-only metadata, with explicit diagnostics.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
