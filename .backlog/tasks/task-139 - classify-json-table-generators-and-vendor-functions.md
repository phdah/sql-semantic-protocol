---
id: TASK-139
title: Classify JSON table generators and vendor functions
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-24'
labels: []
milestone: m-3
dependencies:
  - TASK-138
references:
  - TASK-77
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-77 child 3/4.** Implement only classify json table generators and vendor functions in a focused PR against `main` after dependencies merge. The original TASK-77 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-77 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `77-C139-P`, `77-C139-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** JSON_TABLE/FLATTEN/EXPLODE; generator/table UDF; variable shape.
- **Independent concrete oracle baseline:** For declared array [5,6], documented deterministic flatten yields 2 elements; missing shape => residual.
- **Fail-closed boundary and owner:** No unknown function cardinality or JSON schema guesses. TASK-139 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-77 AC:** #1, #2, #3, #5, #6.

## Verified acceptance and final signoff (2026-10-24)

The [versioned v3 fixture inventory](../../docs/task-139-acceptance-freeze.json)
and [independent acceptance audit](../../docs/task-139-acceptance-audit.md)
cover all named operator families: deterministic, typed source-free
`EXPLODE`, conditional `JSON_TABLE` and Snowflake `FLATTEN`,
opaque generator/table UDFs and variable-shape generators. No mandatory
fixture is left Residual. JSON, vendor/session/runtime and opaque function
laws without authoritative schema and source evidence have explicit,
reviewed, TASK-139-owned Residual boundaries, not invented Exact output.

- **AC #1:** Both stable fixture families, three mandatory and four conditional
  finite cases, 91 exact/parser/residual dialect classifications and 21
  SQL/dbt/ODCS adapter cells are frozen with typed output domains and
  closed-world positive/negative expectations. The 91 cells comprise
  33 Exact, 57 Residual and one parser boundary.
- **AC #2:** `src/analysis/finite_tables.rs` normalizes supported source-free
  `EXPLODE` forms to the canonical `FiniteTableKind::Explode`.
  The physical proof preserves duplicate row identities, SQL NULL,
  exact integer bounds, positive Exact and absent-row Impossible.
  Unknown sources or unproved generator laws remain Residual.
- **AC #3:** `tests/task_139_frozen_matrix.rs` and
  `tests/task_139_generators.rs` assert exact complete DuckDB
  logical-array source and terminal-output oracles, independent expected
  physical identities, typed bounds, nullability and positive/NOT TRUE
  membership, including duplicate, NULL and ill-typed counterexamples.
- **AC #4:** `tests/task_139_frozen_matrix.rs` certifies the full
  13-dialect parser matrix; `tests/task_139_adapter_matrix.rs`
  tests all 21 SQL/dbt/ODCS classifications and canonical wire parity.
  Real dbt Core materialized DuckDB models and complete execution
  oracles are exercised in `tests/task_139_dbt_core_e2e.rs`.
  A DuckDB UNNEST array oracle is not native vendor FLATTEN or
  EXPLODE certification, and no such runtime evidence is claimed.
- **AC #5:** Final implementation-head [CI 40132251800](https://github.com/phdah/sql-semantic-protocol/actions/runs/40132251800)
  passed all **9/9** checks: rustfmt, strict Clippy, complete/default
  and no-default-features Rust tests, docs, dependency boundary,
  real dbt Core E2E and golden/schema snapshots. The audit records
  independent source/terminal assertions and two-axis standards/spec review.

**Child TASK-139 Done.** PR #146 is the delivery PR; TASK-140 remains
blocked until PR #146 is merged on `main`. TASK-77 parent original
AC #1..#6 and protocol v3 release gates remain open and independently
verified later; this child does not approve either.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-139; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-77 parent open.
<!-- AC:END -->

## Next

Next: [TASK-140](task-140%20-%20compose-table-fanout-with-joins-dbt-and-certify-task-77.md), only after this child is Done and merged.
