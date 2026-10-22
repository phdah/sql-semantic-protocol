---
id: TASK-129
title: Prove EXISTS and NOT EXISTS correlated/uncorrelated
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-22'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
references:
  - TASK-75
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-75 child 1/4.** Implement only prove exists and not exists correlated/uncorrelated in a focused PR against `main` after dependencies merge. The original TASK-75 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-75 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `75-C129-P`, `75-C129-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** single-key correlation; duplicate inner bags; empty inner; TRUE/NOT TRUE absence.
- **Independent concrete oracle baseline:** outer keys [1,2,NULL], inner [1,1,NULL] => EXISTS one outer row, NOT EXISTS two.
- **Fail-closed boundary and owner:** No existential witness may substitute for closed-world NOT EXISTS exclusion. TASK-129 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-75 AC:** #1, #2, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-129; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-75 parent open.
<!-- AC:END -->

## Implementation evidence

- [TASK-129 frozen finite matrix](../../docs/task-129-acceptance-freeze.json) fixes six correlated/uncorrelated EXISTS and NOT EXISTS positive/NOT TRUE cases across two complete physical sources, including duplicate and NULL inner keys and empty inner bags. All 13 dialect parser paths and SQL/dbt/ODCS adapter classifications must be independently executed before marking any criterion complete.
- [Complete finite physical evaluator](../../src/physical_realization/exists_bags.rs) reuses canonical normalized subquery witnesses and schema checks; its outputs include stable selected/rejected row identities, all inner candidates, full terminal bags, and exact nullable integer domains.
- [Public DuckDB and dialect oracles](../../tests/task_129_exists_bags.rs), [SQL/dbt/ODCS parity](../../tests/task_129_adapter_parity.rs), and [draft PR #140](https://github.com/phdah/sql-semantic-protocol/pull/140) record implementation progress.
- **Verification:** All five bounded child AC have been independently verified on the final semantic head, with the freeze, audit and all-green CI cited below. TASK-75 parent signoff and release approval remain separate.

## Final independent acceptance signoff (2026-10-22)

- All five bounded AC signed off against the [versioned six-case frozen matrix](../../docs/task-129-acceptance-freeze.json) and [independent acceptance audit](../../docs/task-129-acceptance-audit.md). Includes 78 validated parser/canonical dialect cells and 18 SQL/dbt/ODCS adapter cells.
- All complete DuckDB physical source, terminal, selected and NOT TRUE row bags and their typed nullable domains are independently asserted. Correlated duplicate keys, uncorrelated existence, empty nested bags and SQL NULL comparisons prove the frozen numerical baseline.
- [Semantic-head CI run 39791120871](https://github.com/phdah/sql-semantic-protocol/actions/runs/39791120871) passed all nine project jobs, including format, strict Clippy, default/no-default Rust tests, docs, dependencies and dbt E2E. Native vendor/session behavior remains uncertified and explicitly owned TASK-129 residual; no silent portable-to-native promotion.
- **Merge gate:** child Done records verified bounded acceptance, not a merged predecessor. TASK-130 stays blocked until PR #140 merges. TASK-75 parent original criteria and release gate remain unchecked.

## Next

Next: [TASK-130](task-130%20-%20prove-in-and-not-in-three-valued-membership.md), only after this child is Done and merged.
