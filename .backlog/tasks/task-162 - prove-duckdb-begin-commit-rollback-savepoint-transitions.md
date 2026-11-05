---
id: TASK-162
title: Prove DuckDB BEGIN COMMIT ROLLBACK SAVEPOINT transitions
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-05'
labels: []
milestone: m-3
dependencies:
  - TASK-161
references:
  - TASK-84
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-84 child 2/4.** Implement prove duckdb begin commit rollback savepoint transitions in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-84.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `84-C162-P`, `84-C162-N`.
- **Named mandatory/conditional variants:** BEGIN/COMMIT; ROLLBACK; SAVEPOINT/release/rollback-to; negative replay.
- **Numerical/row-state oracle:** BEGIN INSERT key1 ROLLBACK => empty table; BEGIN INSERT key2 COMMIT => one row.
- **Typed conditional boundary:** Only certify SAVEPOINT when concrete DuckDB engine supports and oracle verifies it. Owner: TASK-162.
- **Original TASK-84 criteria:** #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-162 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-84 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-163](task-163%20-%20classify-auto-commit-implicit-ddl-and-engine-isolation-boundaries.md) after this child is merged.

## Bounded child acceptance signoff (2026-11-05)

- **Status:** TASK-162 Done with bounded technical signoff; TASK-84 parent stays To Do. TASK-163 is not unblocked until PR #189 merges on main.
- **PR:** https://github.com/phdah/sql-semantic-protocol/pull/189
- **Frozen scope:** `docs/task-162-acceptance-freeze.json` v3; immutable v1 and v2 retained. 10 frozen cases, 8 required classifications and 2 conditional Residual boundaries, 91 dialect parser cells across all 13 exposed names.
- **Implementation/oracles:** `src/analysis/transactions.rs`, `src/physical_realization/transactions.rs`, `tests/task_162_{transactions,sql_frontend,adapter_parity,dialect_matrix,acceptance_inventory}.rs`. Exact typed source/intermediate/terminal bags, NULL and duplicates, read TRUE/NOT TRUE, constraint enforcement and rollback restoration; invalid controls Impossible.
- **Conditional boundaries:** Native DuckDB 1.10506.0 does not execute SAVEPOINT/ROLLBACK TO/RELEASE, hence Residual owned by TASK-162; separately executed ALTER defaults remain explicitly Residual pending TASK-164. Neither is promoted to Exact.
- **Verified implementation-head CI:** https://github.com/phdah/sql-semantic-protocol/actions/runs/42109605476, 9/9 successful at `3d85a7806981118473cb92fb0935a3e56cec6c2d` (fmt, all-features/no-default strict lint and tests, docs, dependencies, dbt E2E). Active protocol root/schema unchanged.
- **Two-axis review:** https://github.com/phdah/sql-semantic-protocol/pull/189#pullrequestreview-5792140694. Standards PASS and Spec PASS with no blocking code findings. Review submitted from connected PR author's GitHub account, **not an independent second-person review**. The requested owner-directed author-account exception is disclosed in `docs/task-162-acceptance-audit.md`; separate human review may still be required by repository merge policy.
- **Parent gate:** TASK-84 remains open for TASK-163 and TASK-164 plus its original acceptance audit. GitHub merge is separate from child Done.
