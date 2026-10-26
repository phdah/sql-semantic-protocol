---
id: TASK-135
title: Compose nested page boundaries through CTE and sets
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-26'
labels: []
milestone: m-3
dependencies:
  - TASK-134
references:
  - TASK-76
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-76 child 3/4.** Implement only compose nested page boundaries through cte and sets in a focused PR against `main` after dependencies merge. The original TASK-76 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-76 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `76-C135-P`, `76-C135-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** nested LIMIT+OFFSET CTE; set branch ORDER/LIMIT; producer copies.
- **Independent concrete oracle baseline:** Input [1,2,3,4,5], inner LIMIT 4 then outer OFFSET 2 LIMIT 1 => [3].
- **Fail-closed boundary and owner:** No rewriting offset windows across unresolved producer order boundaries. TASK-135 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-76 AC:** #2, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-135; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-76 parent open.
<!-- AC:END -->

## Next

Next: [TASK-136](task-136%20-%20classify-sampling-and-certify-task-76-dialect-oracle-matrix.md), only after this child is Done and merged.

## Acceptance sign-off

**Decision: Done.** All five originally numbered TASK-135 acceptance criteria are
verified against the fixed six mandatory physical source/terminal bags and
three reviewed conditional Residual variants. No mandatory variant is deferred.

- Frozen typed inventory: [`docs/task-135-acceptance-freeze.json`](../../docs/task-135-acceptance-freeze.json) v2, with 65 parser/canonical dialect paths and 18 SQL/dbt/ODCS adapter cells.
- Independent source/terminal and NULL/duplicate/rejection oracles:
  `tests/task_135_nested_pages.rs`,
  `tests/task_135_set_branch_pages.rs`, plus their adapter suites.
- Actual dbt Core seed/run/docs and compiled-model proof:
  `tests/task_135_dbt_core_e2e.rs`.
- Independent [acceptance audit](../../docs/task-135-acceptance-audit.md)
  includes Matt Pocock Standards/Spec review, the fail-closed
  WHERE-before-page check, case-by-case obligations and original TASK-76 mapping.
- All 9/9 required jobs passed on implementation SHA
  `445b92ac0cd50d9e226832a55446f9fc6bb0cf68`:
  [CI run 40317980536](https://github.com/phdah/sql-semantic-protocol/actions/runs/40317980536).
  The subsequent metadata-only PR head must also pass CI before merge.

**Parent gate:** TASK-76 remains To Do; its six original acceptance criteria
require TASK-136 and independent parent closure. TASK-136 is next only
after PR #150 merges.
