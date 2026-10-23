---
id: TASK-137
title: Prove VALUES and array table fanout with ordinality
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-66
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
references:
  - TASK-77
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-77 child 1/4.** Implement only prove values and array table fanout with ordinality in a focused PR against `main` after dependencies merge. The original TASK-77 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-77 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `77-C137-P`, `77-C137-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** multirow VALUES; array UNNEST/EXPLODE; NULL/empty arrays; ordinality.
- **Independent concrete oracle baseline:** VALUES (1),(2) => 2 rows; UNNEST([1,2])=>2; UNNEST([])=>0; ordinality [1,2].
- **Fail-closed boundary and owner:** Typed nested element/NULL evidence required; opaque collection stays residual. TASK-137 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-77 AC:** #1, #2, #3, #5, #6.

## Current implementation evidence

PR [#144](https://github.com/phdah/sql-semantic-protocol/pull/144) implements
canonical, source-free VALUES and literal-array UNNEST/EXPLODE row facts in
`src/protocol/finite_table.rs` and `src/analysis/finite_tables.rs`.
`src/physical_realization/finite_tables.rs` classifies complete terminal bags,
typed domains and positive/absent identity membership. The corrected
[v2 fixture matrix](../../docs/task-137-acceptance-freeze.json) preserves the
[original frozen v1](../../docs/task-137-acceptance-freeze-v1.json).
`tests/task_137_frozen_matrix.rs` covers the seven required source-free
bags, full DuckDB oracles, absent candidates and 104 parser paths;
`tests/task_137_adapter_parity.rs` checks compiled dbt parity.

Acceptance remains open until final-head tests and independent review establish
the mandatory parser/native and adapter boundaries. Typed source-column fanout
and composed joins/aggregations belong to TASK-140. Unproved vendor
EXPLODE table-generator semantics must not be certified from parser acceptance
or equivalent UNNEST execution alone.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-137; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-77 parent open.
<!-- AC:END -->

## Next

Next: [TASK-138](task-138%20-%20implement-star-transforms-and-fixed-schema-pivot-unpivot.md), only after this child is Done and merged.
