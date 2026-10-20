---
id: TASK-126
title: Prove navigation and bounded ROWS frames
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-20'
labels: []
milestone: m-3
dependencies:
  - TASK-125
references:
  - TASK-74
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-74 child 2/5 (originally 2/4).** Implement only prove navigation and bounded rows frames in a focused PR against `main` after dependencies merge. The original TASK-74 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-74 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `74-C126-P`, `74-C126-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** LAG/LEAD; FIRST_VALUE/LAST_VALUE; bounded ROWS frame; NULLs; frame boundaries.
- **Independent concrete oracle baseline:** ordered values [10,NULL,30]: LAG row2=10, LEAD row2=30; ROWS 1 PRECEDING running COUNT(*)=[1,2,2].
- **Fail-closed boundary and owner:** Other RANGE/GROUPS/NTILE/collation variants classified separately. TASK-126 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-74 AC:** #1, #2, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-126; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-74 parent open.
<!-- AC:END -->

## Next

Next: [TASK-189](task-189%20-%20prove-common-aggregate-window-functions.md), only after this child is Done and merged. TASK-127 follows TASK-189.

## Verification and signoff

Independently verified on 2026-10-20 against the immutable v1 matrix at `docs/task-126-acceptance-freeze.json`. The review, original AC crosswalk, all 104 dialect parser cells, 24 adapter classifications, exact DuckDB bags and CI evidence are recorded in `docs/task-126-acceptance-audit.md`.

All five child AC are checked. CI run [3316](https://github.com/phdah/sql-semantic-protocol/actions/runs/39411757453) passed all jobs on the verified semantic/test head; the final metadata-only head is subject to CI as a pre-merge gate. TASK-74 remains open, PR #136 remains unmerged, and TASK-189 cannot start before this child merges on `main`.
