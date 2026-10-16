---
id: TASK-88
title: Verify all exposed SQL dialects and dialect-specific semantic laws
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-70
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-77
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
references: 
  - 'TASK-47'
  - 'TASK-51'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Parser support does not prove executed semantics. A release-quality protocol needs a feature-by-dialect conformance map with comparison and DML/DDL differences captured.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Drive the canonical feature inventory for all exposed dialects: ANSI, BigQuery, ClickHouse, Databricks, DuckDB, generic, Hive, MSSQL, MySQL, PostgreSQL, Redshift, Snowflake and SQLite.
- [ ] #2 Check shared AST shapes across all parse-capable dialects and focused variants such as QUALIFY, TOP, FETCH, MINUS, BY NAME sets, UPSERT, INSERT OVERWRITE, MERGE, ASOF/APPLY and CREATE OR REPLACE.
- [ ] #3 Declare laws for NULL ordering, bag/tuple equality, implicit coercion, identifier casing, collations, floating/timestamp semantics, transaction behavior and DML match multiplicity.
- [ ] #4 Distinguish parse, canonical proof, generator-ready witness and actual engine execution. Require real-engine oracle for each supported engine claim; mark other engines unverified.
- [ ] #5 Fail CI on missing matrix entries, unreviewed residuals and any newly parsed form incorrectly treated as exact.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
- [ ] #7 Require CI parser and canonical-equivalence tests for **all 13 exposed dialects** and every reviewed shared/dialect-specific supported variant. For semantically equivalent SQL, compare exact parser-independent protocol outcomes, value-domain bounds/inclusivity, lineage, membership proof, cardinality, write effects and residual state, excluding only dialect provenance metadata.
- [ ] #8 When SQL syntax or engine semantics genuinely differ, declare explicit dialect/session assumptions and equivalence mappings; block any unverified advertised semantic support. Native vendor-engine execution evidence remains distinct from parsing/canonical proof and DuckDB oracles.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#8 remain required**. Interpret and verify them through the frozen TASK-88 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Complete inventory: all 13 project dialects crossed with the approved TASK-66 feature/variant IDs and parser statuses; no implicit skipping of parse-capable variants.
2. **Mandatory cross-operator / adapter evidence:** Exact equivalence: for each approved shared semantic class assert equal canonical domains, lineage, membership, cardinality and write effects, with minimal AST-specific SQL fixtures.
3. **Conditional and non-exact boundary:** Conditional boundaries: vendor-dependent NULL order, collation, floating/timezone, coercion, DML match/transaction rules get version/session assumptions; native engine claims only when executed.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#8 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Maintainer decision (2026-10-09)

Approved [13-dialect canonical-equivalence acceptance](../../docs/coverage-signoff.md): parsing runs in CI without vendor databases and equivalent SQL in every supported dialect must emit **semantically identical canonical protocol outcomes**, not only parse. DuckDB executes the equivalent common SQL and validates actual generated data. Externally unavailable engines must never be marked execution-certified. Fixture baseline tests in TASK-66 are representative, not sufficient for TASK-88 acceptance.
