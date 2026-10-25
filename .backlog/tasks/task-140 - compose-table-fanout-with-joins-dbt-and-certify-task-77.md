---
id: TASK-140
title: Compose table fanout with joins/dbt and certify TASK-77
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-25'
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
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-140; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-77 parent open.
<!-- AC:END -->

## Next

Then independently sign off all original TASK-77 AC against the entire merged child evidence.

## Verified implementation evidence

The versioned [TASK-140 nine-case acceptance matrix](../../docs/task-140-acceptance-freeze.json) certifies 7 mandatory exact and 2 reviewed conditional variants, 117 dialect cells (72 Exact, 36 typed Residual and 9 parser boundaries) and 27 adapter cells. Its complete leaf source bags, selected/SQL FALSE/SQL UNKNOWN physical pairs, duplicate row identities, inclusive typed output domains, NULL and empty-array laws, terminal COUNT(*) bag and requested Impossible membership agree with independent native DuckDB execution. SQLite/MSSQL accepted parser forms without equivalent attested native array execution remain explicit, TASK-140-owned conditional runtime boundaries rather than fabricated portable engine facts.

The actual compiled dbt Core models are executed in DuckDB and compared to direct SQL and canonical physical proofs. Equivalent ODCS schema evidence cannot supply literal/array row values; the adapter cannot fabricate an exact population.

All five bounded child criteria were independently reviewed against [the TASK-140 acceptance audit](../../docs/task-140-acceptance-audit.md) and validated on implementation code head `b8a0a01461f22e16a13e3ea8de8d463df61a98fd`. The final implementation [CI run](https://github.com/phdah/sql-semantic-protocol/actions/runs/40242206911) passed all 9 required jobs, including formatting, strict Clippy, default and no-default full tests, documentation and actual dbt Core E2E.

**TASK-140 is Done pending PR #149 merge.** This does not sign off TASK-77's original six AC, which need an independent parent review after all four child PRs are merged. The TASK-91 release gate remains independent.

## Handoff

Merge [PR #149](https://github.com/phdah/sql-semantic-protocol/pull/149) only after its metadata-only final-head CI succeeds. Then independently reconcile the merged TASK-137..140 evidence against TASK-77 AC #1..#6 and keep the parent open until all gates are verified.
