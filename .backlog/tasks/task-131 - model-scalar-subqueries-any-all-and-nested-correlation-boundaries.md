---
id: TASK-131
title: Model scalar subqueries ANY ALL and nested correlation boundaries
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-23'
labels: []
milestone: m-3
dependencies:
  - TASK-130
references:
  - TASK-75
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-75 child 3/4.** Implement only model scalar subqueries any all and nested correlation boundaries in a focused PR against `main` after dependencies merge. The original TASK-75 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-75 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `75-C131-P`, `75-C131-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** scalar subquery zero/one/multiple rows; ANY/SOME/ALL; nesting; LATERAL/APPLY.
- **Independent concrete oracle baseline:** scalar subquery empty returns NULL; singleton [2] returns 2; two rows raise cardinality error in DuckDB.
- **Fail-closed boundary and owner:** Runtime errors, engine-specific quantification and APPLY unsupported paths have named fail-closed reasons. TASK-131 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-75 AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-131; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-75 parent open.
<!-- AC:END -->

## Next

Next: [TASK-132](task-132%20-%20compose-grouped-joined-correlations-and-independently-certify-task-75.md), only after this child is Done and merged.

## Final TASK-131 sign-off (2026-10-23)

- All five child acceptance criteria verified against the **v5 immutable case inventory**, with 12 mandatory cases and three explicitly owned conditional cases. The v5 correction to N6/N7 complete physical source bags was reviewed, and no mandatory proof was weakened.
- Canonical SQL, source-attested scalar/ANY/SOME/ALL, three-valued and typed domain proofs are covered by `tests/task_131_scalar_quantified.rs`; independent native DuckDB source/terminal evidence is fixed to the freeze, with a complete public JSON golden snapshot.
- All 195 parser and 45 adapter cells are covered/classified, including nine conditional SQL/dbt/ODCS residual classifications. Native DuckDB execution is the sole engine runtime oracle, not proof of vendor execution elsewhere.
- Independent acceptance and code-standards review: [audit](../../docs/task-131-acceptance-audit.md). Final verified implementation head: `f7c050442c7fd1a3ce5dcf2b1bf6e373994aedc3`, [CI #39916461940](https://github.com/phdah/sql-semantic-protocol/actions/runs/39916461940) (all required jobs passed).
- TASK-75 remains open. TASK-132 is blocked until this child is merged into `main`. This child does not authorize or claim a v3 release.
