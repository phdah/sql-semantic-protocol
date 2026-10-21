---
id: TASK-127
title: Compose QUALIFY and upstream grouped or joined row shaping
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-21'
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
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-127; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-74 parent open.
<!-- AC:END -->

## Next

Next: [TASK-128](task-128%20-%20certify-advanced-frames-vendor-laws-and-task-74-signoff.md), only after this child is Done and merged.

## Final acceptance sign-off (2026-10-21)

All five child acceptance criteria independently verified for the frozen TASK-127 scope. The original SQL, physical-source bag and typed terminal-domain fixtures were frozen before semantic changes; seven mandatory variants are exact, one underordered peer case is intentionally Residual. All 104 parser/canonical dialect cells and 24 SQL/dbt/ODCS adapter cells have verified classifications. Independent DuckDB source and terminal oracles, exact joined/grouped producer identities, complete physical contributors, SQL NULL/rejection cases and strict negative boundaries are covered by the public integration tests.

**Implementation-head CI:** [run 39674727466](https://github.com/phdah/sql-semantic-protocol/actions/runs/39674727466), commit `294175da8b30280baf2cec8a52b54e5b0ba4ecd6`, passed all **nine** jobs (format, strict lint, full tests, docs and no-default equivalents, dbt E2E and schema/golden regression tests). Closure metadata requires its own last-PR-head green CI before merge.

Independent evidence and non-promotion boundaries: [task-127-acceptance-audit.md](../../docs/task-127-acceptance-audit.md). The TASK-74 parent's original AC remain unchecked pending all children and separate review; TASK-128 and protocol 3.0.0 release approval are **not** signed off by this task.
