---
id: TASK-140
title: Compose table fanout with joins/dbt and certify TASK-77
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-139
references:
  - TASK-77
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-77 child 4/4.** Implement only compose table fanout with joins/dbt and certify task-77 in a focused PR against `main` after dependencies merge. The original TASK-77 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-77 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `77-C140-P`, `77-C140-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** VALUES/UNNEST fanout feeding join or group; dbt adapter; 13 dialects.
- **Independent concrete oracle baseline:** UNNEST([1,1,2]) GROUP BY value => groups 1:2, 2:1; negative absent group verified.
- **Fail-closed boundary and owner:** Every deterministic variant has full source bag and terminal snapshot; conditional laws explicit. TASK-140 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-77 AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-140; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-77 parent open.
<!-- AC:END -->

## Next

Then independently sign off all original TASK-77 AC against the entire merged child evidence.
