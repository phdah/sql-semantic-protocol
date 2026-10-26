---
id: TASK-134
title: Prove ties DISTINCT and QUALIFY pagination boundaries
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-25'
labels: []
milestone: m-3
dependencies:
  - TASK-133
references:
  - TASK-76
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-76 child 2/4.** Implement only prove ties distinct and qualify pagination boundaries in a focused PR against `main` after dependencies merge. The original TASK-76 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-76 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `76-C134-P`, `76-C134-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** WITH TIES; DISTINCT; DISTINCT ON; SQL NULL ordering.
- **Independent concrete oracle baseline:** ORDER BY key DESC values [3,2,2,1] FETCH FIRST 2 WITH TIES => [3,2,2] count 3.
- **Fail-closed boundary and owner:** Unspecified NULL ordering and DISTINCT ON tie choice must be typed residual. TASK-134 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-76 AC:** #1, #2, #3, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-134; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-76 parent open.
<!-- AC:END -->

## Next

Next: [TASK-135](task-135%20-%20compose-nested-page-boundaries-through-cte-and-sets.md), only after this child is Done and merged.

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Canonical direct `WITH TIES`, `DISTINCT` and deterministically provable `DISTINCT ON` complete physical bags are certified without skipping source producers.
- `docs/task-134-acceptance-freeze.json` version 1 owns six mandatory exact and three conditional residual cases, with stable fixture families `76-C134-P` and `76-C134-N`.
- The verified test matrix includes 78 exact parser cells, 39 conditional residual parser cells, 27 SQL/dbt/ODCS adapter cells, independent complete DuckDB bags and an executed compiled dbt Core model.
- Conditional implicit NULL ordering, ambiguous DISTINCT ON winner identity and QUALIFY producer composition remain explicit owner-reviewed Residual; no native warehouse execution is claimed.
- The original TASK-76 parent and TASK-91 release gates remain independent and open.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
All five TASK-134 acceptance criteria passed independent semantic and source-level acceptance review on commit `64bb50288fcc58704f403718a65ca89f62b8255a`. GitHub Actions run [40228675764](https://github.com/phdah/sql-semantic-protocol/actions/runs/40228675764) passed all nine required checks, including strict lint, full Rust and no-default-feature suites, dbt Core execution, active schema/golden tests, docs, and formatting. See `docs/task-134-acceptance-audit.md` for case-by-case evidence and scope boundaries.

Delivery PR: https://github.com/phdah/sql-semantic-protocol/pull/148. This final metadata-only task closure must pass its own PR-head CI before merge. TASK-135 remains blocked until TASK-134 is Done **and PR #148 is merged into main**; TASK-76 parent stays open.
<!-- SECTION:FINAL_SUMMARY:END -->
