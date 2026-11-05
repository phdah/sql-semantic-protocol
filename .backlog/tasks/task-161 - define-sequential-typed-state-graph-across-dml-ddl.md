---
id: TASK-161
title: Define sequential typed state graph across DML DDL
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-05'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-78
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
references:
  - TASK-84
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-84 child 1/4.** Implement define sequential typed state graph across dml ddl in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-84.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `84-C161-P`, `84-C161-N`.
- **Named mandatory/conditional variants:** INSERT/UPDATE/DELETE/MERGE/DDL; per-step before/after, producer/read identity.
- **Numerical/row-state oracle:** empty T -> INSERT [1,2] -> UPDATE key1 -> DELETE key2 => final one updated row and four state snapshots.
- **Typed conditional boundary:** No free synthetic intermediate state or reorder of read/write dependencies. Owner: TASK-161.
- **Original TASK-84 criteria:** #1, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-161 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-84 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-162](task-162%20-%20prove-duckdb-begin-commit-rollback-savepoint-transitions.md) after this child is merged.

## Implementation PR (2026-11-05)

- Draft PR: https://github.com/phdah/sql-semantic-protocol/pull/188
- Frozen task-owned inventory: `docs/task-161-acceptance-freeze.json`.
- Canonical typed before/after graph and physical producer edges: `src/physical_realization/sequential_graph.rs`.
- Full-bag native DuckDB and negative tests: `tests/task_161_sequential_graph.rs`.
- Bounded child TASK-161 is Done after all mandatory source/state/adapter/dialect oracles and 9/9 implementation-head CI pass. A connected-author-account two-axis review was recorded, with its non-independent reviewer provenance and owner-directed exception disclosed in the acceptance audit. TASK-84 stays To Do; TASK-162 cannot start until this child is merged into main.

## Final bounded child signoff (2026-11-05)

- Original child AC #1..#5 checked against active frozen v2 (immutable original v1), 6 mandatory cases and 1 conditional transaction/session boundary.
- Full DuckDB oracles plus original 4-state example and initial source-producer identity: `tests/task_161_{sequential_graph,initial_provenance,full_oracles,four_snapshots}.rs`.
- All 13 dialects, 91 pinned case cells, 351 parser statement cells: `tests/task_161_dialect_matrix.rs`.
- SQL/dbt/ODCS representable parity: `tests/task_161_adapter_parity.rs`; conditional transaction semantics belong to TASK-162.
- CI: https://github.com/phdah/sql-semantic-protocol/actions/runs/42094557209, 9/9 jobs successful at `d14171dd247ebc3a2532dc1eed05942aa3176c61`.
- Two-axis review: https://github.com/phdah/sql-semantic-protocol/pull/188#pullrequestreview-5790920674, authored via the connected PR author account (not a second independent human). That exception is disclosed explicitly in `docs/task-161-acceptance-audit.md`.
- This task's Done state does not unblock TASK-162 until PR #188 is **merged**. Parent TASK-84 and TASK-91 remain independent.
