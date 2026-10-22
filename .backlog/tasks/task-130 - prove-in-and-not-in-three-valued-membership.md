---
id: TASK-130
title: Prove IN and NOT IN three-valued membership
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-22'
labels: []
milestone: m-3
dependencies:
  - TASK-129
references:
  - TASK-75
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-75 child 2/4.** Implement only prove in and not in three-valued membership in a focused PR against `main` after dependencies merge. The original TASK-75 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-75 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `75-C130-P`, `75-C130-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** correlated/uncorrelated IN/NOT IN; NULL inner or outer; compound key.
- **Independent concrete oracle baseline:** outer keys [1,2,NULL], inner [1,NULL] => IN TRUE for 1; NOT IN TRUE for none; 2 and NULL are UNKNOWN.
- **Fail-closed boundary and owner:** NULL UNKNOWN and duplicate tuple membership separate from impossible witnesses. TASK-130 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-75 AC:** #1, #2, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-130; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-75 parent open.
<!-- AC:END -->

## Next

Next: [TASK-131](task-131%20-%20model-scalar-subqueries-any-all-and-nested-correlation-boundaries.md), only after this child is Done and merged.

## Implementation progress (2026-10-22)

A focused implementation PR introduces a complete two-source signed-integer scalar
`IN`/`NOT IN` physical evaluator and records six fixed positive/NOT TRUE
snapshots in [the TASK-130 acceptance inventory](../../docs/task-130-acceptance-freeze.json).
The public [integration regression](../../tests/task_130_in_bags.rs) asserts
per-row SQL TRUE/FALSE/UNKNOWN, full candidate and terminal bag obligations,
nullable typed domains and a separately executed DuckDB SQL oracle.

**Not signed off:** compound-key tuple membership is not yet normalized by the
canonical expression model; dbt/ODCS paired parity and final full CI/independent
review remain mandatory. These gaps block AC #1..#5, TASK-130 Done, and TASK-131
start. No unverified mandatory cell is approved as a residual.
