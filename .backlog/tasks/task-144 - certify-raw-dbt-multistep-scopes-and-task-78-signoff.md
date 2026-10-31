---
id: TASK-144
title: Certify raw/dbt multistep scopes and TASK-78 signoff
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-31'
labels: []
milestone: m-3
dependencies:
  - TASK-143
references:
  - TASK-78
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-78 child 4/4.** Implement only certify raw/dbt multistep scopes and task-78 signoff in a focused PR against `main` after dependencies merge. The original TASK-78 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-78 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `78-C144-P`, `78-C144-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** views, reused sources, multi statements, selective row-changing materialization; dialects.
- **Independent concrete oracle baseline:** DuckDB full initial/producer/output snapshots for shadowed CTE, bounded recursion and two-consumer filter.
- **Fail-closed boundary and owner:** Unsupported LATERAL/temporary session scope typed residual with evidence and owner. TASK-144 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-78 AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-144; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-78 parent open.
<!-- AC:END -->

## Next

Then independently sign off all original TASK-78 AC against the entire merged child evidence.

## Bounded child signoff (2026-10-31)

All five child criteria are verified against the independent
[acceptance audit](../../docs/task-144-acceptance-audit.md). The immutable
preimplementation v1 freeze and reviewed explicit v2 matrix cover seven
mandatory Exact cases, two conditional typed Residual boundaries, 126
parser/canonical dialect cells, and 27 direct SQL/dbt/ODCS dispositions.
Complete canonical and independent native DuckDB source, lexical producer,
materialized target and terminal bags assert exact physical row identities,
SQL NULL and duplicate multiplicity, inclusive value domains and absence
proofs. Ordinary logical VIEW is re-evaluated from physical leaf rows; unproved
temporary/materialized/session laws remain recorded and non-Exact.

The semantic implementation head `0a05bc16d598c99bd5961f6536c6ce78b9a53e27`
passed all nine required CI jobs in
[run 41222021788](https://github.com/phdah/sql-semantic-protocol/actions/runs/41222021788).
A separate Matt two-axis Standards/Spec audit found no further blocking
TASK-144 defects:
[PR review](https://github.com/phdah/sql-semantic-protocol/pull/170#pullrequestreview-5723914446)
(authored COMMENT, not independent-account approval). The final audit/task
metadata commit must also pass latest-head CI before PR merge.

**Child:** Done on this PR branch. **TASK-78 parent:** remains To Do
until this child PR merges into `main` and the original parent AC #1..#6
are independently closed. No release approval or unverified native engine
certification is implied.
