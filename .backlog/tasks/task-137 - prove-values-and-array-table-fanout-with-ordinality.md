---
id: TASK-137
title: Prove VALUES and array table fanout with ordinality
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-24'
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

## Verification and acceptance evidence

The complete, source-free literal VALUES and UNNEST/EXPLODE model has canonical
typed row facts (`src/protocol/finite_table.rs`), AST normalization
(`src/analysis/finite_tables.rs`) and complete physical bag, row identity,
domain and membership proofs (`src/physical_realization/finite_tables.rs`).
The versioned [v3 acceptance inventory](../../docs/task-137-acceptance-freeze.json)
retains the immutable v1 and v2 inventories and their reviewed corrections.

All seven mandatory fixtures have independent full-bag DuckDB oracles with
exact domains, multiplicity, positive identity, and Impossible absent-row proofs.
All 104 dialect parser paths and 24 direct SQL/dbt/ODCS adapter cells have
explicitly verified classifications. The real dbt Core warehouse and catalog
E2E oracle passes, along with Rust tests, fmt, Clippy, docs, no-default-feature
checks and active schema/golden snapshots. An independent semantic/spec and
canonical code review is recorded in
[the acceptance audit](../../docs/task-137-acceptance-audit.md).

Opaque collection and vendor/session generator semantics remain reviewed,
explicitly owned TASK-137 conditional residuals, not mandatory implementation
gaps. Source-column fanout and composed joins/aggregations belong to TASK-140.
No TASK-77 parent AC or protocol v3 release gate is signed off here.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-137; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-77 parent open.
<!-- AC:END -->

## Next

Next: [TASK-138](task-138%20-%20implement-star-transforms-and-fixed-schema-pivot-unpivot.md), only after this child is Done and merged.
