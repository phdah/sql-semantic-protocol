---
id: TASK-133
title: Prove deterministic ORDER BY LIMIT OFFSET and TOP
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-25'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-74
references:
  - TASK-76
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-76 child 1/4.** Implement only prove deterministic order by limit offset and top in a focused PR against `main` after dependencies merge. The original TASK-76 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-76 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `76-C133-P`, `76-C133-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** sorted total order; LIMIT/OFFSET; FETCH FIRST; TOP variant.
- **Independent concrete oracle baseline:** keys [1,2,3,4] ascending LIMIT 2 OFFSET 1 => [2,3]; empty input -> empty output.
- **Fail-closed boundary and owner:** No survivor identity without total order or attested ties. TASK-133 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-76 AC:** #1, #2, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-133; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-76 parent open.
<!-- AC:END -->

## Next

Next: [TASK-134](task-134%20-%20prove-ties-distinct-and-qualify-pagination-boundaries.md), only after this child is Done and merged.

## Verified TASK-133 signoff (2026-10-25)

- [Versioned v3 exact/conditional freeze](../../docs/task-133-acceptance-freeze.json): 9 mandatory Exact and 4 conditional typed Residual cases, all 169 dialect and 39 SQL/dbt/ODCS cells executed.
- [Independent acceptance audit](../../docs/task-133-acceptance-audit.md): verifies every AC #1..#5 using complete source/terminal DuckDB bags, typed NULL bounds, stable selected/rejected physical IDs, requested-row membership, impossible source contradiction, real dbt Core seed/run/docs oracle and preserved active schema/golden.
- **9/9 green on semantic implementation head:** [CI run 40212402845](https://github.com/phdah/sql-semantic-protocol/actions/runs/40212402845). Final metadata-head CI must remain green before merging PR #147.
- TASK-76 parent stays open. Native non-DuckDB vendor/session laws are not certified; no 3.0.0 release approval is implied. TASK-134 is dependency-blocked until PR #147 merges on main.
