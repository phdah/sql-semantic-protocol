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
  - TASK-173
  - TASK-174
  - TASK-175
  - TASK-176
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

## Bounded dependency-ordered child execution queue (2026-10-17)

TASK-87 is an **independent parent signoff** and cannot be closed from a narrow passing CI subset. All original AC remain mandatory and unchecked until every child below is Done and merged. Choose first unfinished child with satisfied upstream dependencies; no child depends on TASK-87. `docs/backlog-child-queue-plan.json` is the versioned planning inventory, and each child must register a frozen, executable task-specific acceptance matrix before semantics change.

- **[TASK-173](task-173%20-%20freeze-authoritative-metadata-fact-ownership-and-absence-laws.md)** (1/4): dbt compiled SQL/dependencies/constraints; catalog types; ODCS fields; direct DDL. Fixtures `87-C173-P`, `87-C173-N`.
- **[TASK-174](task-174%20-%20certify-dbt-ephemeral-view-table-and-incremental-semantics.md)** (2/4): dbt materialization tags; producer/write identity; executed vs unexecuted model. Fixtures `87-C174-P`, `87-C174-N`.
- **[TASK-175](task-175%20-%20resolve-schema-aliases-fk-only-source-and-adapter-limitations.md)** (3/4): schema-qualified quoted names; tests; FK-only evidence; missing warehouse metadata. Fixtures `87-C175-P`, `87-C175-N`.
- **[TASK-176](task-176%20-%20certify-sql-dbt-odcs-paired-canonical-parity-across-writes.md)** (4/4): three adapter pairs: table, view/ephemeral, incremental/write; typed source constraints. Fixtures `87-C176-P`, `87-C176-N`.

**Original AC ownership:** #1 -> TASK-173, TASK-174, TASK-175, TASK-176; #2 -> TASK-173, TASK-174, TASK-175, TASK-176; #3 -> TASK-174, TASK-176; #4 -> TASK-173, TASK-175, TASK-176; #5 -> TASK-173, TASK-174, TASK-175, TASK-176; #6 -> TASK-173, TASK-174, TASK-175, TASK-176.

**No premature Done:** mandatory typed exactness cannot be deferred as residual; unverified cells block completion. Positive/NOT TRUE rows, nullable interval output domains, full DuckDB bags, adapter/13-dialect evidence, reviewed conditional vendor cases and final CI are necessary. TASK-91 remains a distinct release decision after TASK-87 closure.
