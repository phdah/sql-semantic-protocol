---
id: TASK-127
title: Compose QUALIFY and upstream grouped or joined row shaping
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-189
references:
  - TASK-74
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-74 child 4/5 (originally 3/4).** Implement only compose qualify and upstream grouped or joined row shaping in a focused PR against `main` after dependencies merge. The original TASK-74 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-74 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `74-C127-P`, `74-C127-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** QUALIFY ROW_NUMBER<=n; multiple rank conditions; joins and grouped inputs; rejected peers.
- **Independent concrete oracle baseline:** two partitions each [3,2,1] ordered descending, QUALIFY ROW_NUMBER=1 gives 2 survivors and 4 rejected.
- **Fail-closed boundary and owner:** Survivors and NOT TRUE witnesses must share exact upstream bag and stable row identities. TASK-127 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-74 AC:** #1, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-127; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-74 parent open.
<!-- AC:END -->

## Next

Next: [TASK-128](task-128%20-%20certify-advanced-frames-vendor-laws-and-task-74-signoff.md), only after this child is Done and merged.
