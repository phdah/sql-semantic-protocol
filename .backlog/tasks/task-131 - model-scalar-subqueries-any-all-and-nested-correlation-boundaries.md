---
id: TASK-131
title: Model scalar subqueries ANY ALL and nested correlation boundaries
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-22'
labels: []
milestone: m-3
dependencies:
  - TASK-130
references:
  - TASK-75
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-75 child 3/4.** Implement only model scalar subqueries any all and nested correlation boundaries in a focused PR against `main` after dependencies merge. The original TASK-75 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-75 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `75-C131-P`, `75-C131-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** scalar subquery zero/one/multiple rows; ANY/SOME/ALL; nesting; LATERAL/APPLY.
- **Independent concrete oracle baseline:** scalar subquery empty returns NULL; singleton [2] returns 2; two rows raise cardinality error in DuckDB.
- **Fail-closed boundary and owner:** Runtime errors, engine-specific quantification and APPLY unsupported paths have named fail-closed reasons. TASK-131 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-75 AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-131; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-75 parent open.
<!-- AC:END -->

## Next

Next: [TASK-132](task-132%20-%20compose-grouped-joined-correlations-and-independently-certify-task-75.md), only after this child is Done and merged.

## Implementation checkpoint (2026-10-22)

- Versioned frozen TASK-131 case inventory: [freeze](../../docs/task-131-acceptance-freeze.json), registered in [coverage](../../docs/coverage-manifest.json) before semantic changes.
- Initial canonical/operator-local physical implementation: `src/physical_realization/scalar_quantified.rs`.
- Public API and independent DuckDB oracles: `tests/task_131_scalar_quantified.rs`.
- Exact physical distinctions: empty scalar is NULL; singleton is its value; multiple source rows raise cardinality error, not Impossible. ANY/SOME/ALL preserve SQL TRUE/FALSE/UNKNOWN, vacuous empty truth, duplicate row identities and source-to-terminal bag membership.
- **Still required before AC signoff:** SQL AST normalization and binding for quantified predicates; a complete mandatory and conditional per-dialect matrix; correlated nested scope/source identities; executable LATERAL/APPLY classification; independent direct/dbt/ODCS parity; full pinned acceptance bag/domain assertions; complete final-head CI, independent audit, and all five AC checks. The helper for ANY/ALL alone is NOT SQL parser-bound semantics or TASK-131 completion.

**Do not mark Done, claim release certification, or start TASK-132 on this checkpoint.**
