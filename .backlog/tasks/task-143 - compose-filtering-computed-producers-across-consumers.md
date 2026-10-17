---
id: TASK-143
title: Compose filtering/computed producers across consumers
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-142
references:
  - TASK-78
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-78 child 3/4.** Implement only compose filtering/computed producers across consumers in a focused PR against `main` after dependencies merge. The original TASK-78 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-78 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `78-C143-P`, `78-C143-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** CTE WHERE; computed aliases; downstream JOIN/count/write; materialization.
- **Independent concrete oracle baseline:** raw [1,2,3] WHERE k>1 => stage [2,3]; two consumers share exactly same stage bag.
- **Fail-closed boundary and owner:** No intermediate source insertion; preserve producer ordering and row filters. TASK-143 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-78 AC:** #1, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-143; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-78 parent open.
<!-- AC:END -->

## Next

Next: [TASK-144](task-144%20-%20certify-raw-dbt-multistep-scopes-and-task-78-signoff.md), only after this child is Done and merged.
