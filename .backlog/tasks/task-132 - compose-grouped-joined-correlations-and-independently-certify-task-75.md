---
id: TASK-132
title: Compose grouped/joined correlations and independently certify TASK-75
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-23'
labels: []
milestone: m-3
dependencies:
  - TASK-131
references:
  - TASK-75
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-75 child 4/4.** Implement only compose grouped/joined correlations and independently certify task-75 in a focused PR against `main` after dependencies merge. The original TASK-75 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-75 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `75-C132-P`, `75-C132-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** composite correlations through JOIN and GROUP inner; subquery_orders.sql; dbt/ODCS.
- **Independent concrete oracle baseline:** outer [1,2], joined inner key [1,1] => EXISTS only 1; NOT EXISTS only 2; full terminal snapshots required.
- **Fail-closed boundary and owner:** No unproved nested-source identity or missing parser dialect cells in parent signoff. TASK-132 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-75 AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-132; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-75 parent open.
<!-- AC:END -->

## Implementation progress

The versioned [TASK-132 acceptance freeze](../../docs/task-132-acceptance-freeze.json) and [acceptance audit](../../docs/task-132-acceptance-audit.md) own seven mandatory cases and their open release disposition. Canonical complete-bag direct, grouped, and three-source INNER JOIN EXISTS/NOT EXISTS proofs are exercised by 13 dialect public API paths, independent DuckDB source/terminal oracles, and direct SQL/dbt/ODCS source-adapter comparison tests. The committed `subquery_orders.sql` has executable dbt producer/source/terminal snapshots but still lacks a canonical end-to-end multi-producer physical realizability proof. Mandatory matrix cell verification and independent final-head acceptance review remain open. No parent TASK-75 signoff or merge is authorized by this child evidence.

## Next

Then independently sign off all original TASK-75 AC against the entire merged child evidence.
