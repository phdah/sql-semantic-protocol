---
id: TASK-128
title: Certify advanced frames vendor laws and TASK-74 signoff
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-22'
labels: []
milestone: m-3
dependencies:
  - TASK-127
references:
  - TASK-74
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-74 child 5/5 (originally 4/4).** Implement only certify advanced frames vendor laws and task-74 signoff in a focused PR against `main` after dependencies merge. The original TASK-74 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-74 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `74-C128-P`, `74-C128-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** NTILE; RANGE/GROUPS; COLLATE; NULL ordering; ranked_orders.sql; 13 dialects.
- **Independent concrete oracle baseline:** Oracle three ranking laws, one navigation, one ROWS frame, all five common aggregate-window families from TASK-189, grouped QUALIFY and explicit parser/session boundaries.
- **Fail-closed boundary and owner:** No exact claim for underordered or engine-dependent windows; parent signoff independent. TASK-128 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-74 AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-128; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-74 parent open.
<!-- AC:END -->

## Next

Then independently sign off all original TASK-74 AC against the entire merged child evidence.

## Implementation progress (2026-10-22)

- Draft [PR #139](https://github.com/phdah/sql-semantic-protocol/pull/139) holds the first bounded physical-law implementation. It implements source-attested deterministic `NTILE`, peer-inclusive unbounded/CURRENT `RANGE` and offset `GROUPS` with exact finite output domains and typed Residual boundaries.
- [Frozen TASK-128 matrix v1](../../docs/task-128-acceptance-freeze.json) registers six physical SQL fixture cases with two initial P/N fixture IDs, seven source rows, exact terminal bags, typed NULL/inclusive domains, all 13 dialect names, adapter entrypoints and named residual owners. These are **initial slice evidence**, not a complete original AC freeze or a signoff.
- [Public source-bag and DuckDB oracle tests](../../tests/task_128_advanced_windows.rs) pass on [CI run #3476](https://github.com/phdah/sql-semantic-protocol/actions/runs/39736141999), including Rust tests (default/no-default), strict lint, format, docs and dbt E2E. This is **not** an executed dbt model comparison for each TASK-128 fixture.
- **Blocking before AC closure:** replace provisional parser `to_verify` cells with executed observed status; independently execute all representable direct/dbt/ODCS adapter paths and required dialect SQL cases; freeze the full parent/operator matrix including COLLATE, default NULL ordering and the `ranked_orders.sql` project fixture; add positive/rejected QUALIFY and Impossible counterexamples; audit every TASK-74 original AC and pending native/session classification. Re-run full CI on final review head. No checks may be marked Done while any required cell is unverified.
- Keep TASK-74 **To Do**, PR draft, and release gate TASK-91 unchanged until independently verified after this child PR is merged.
