---
id: TASK-162
title: Prove DuckDB BEGIN COMMIT ROLLBACK SAVEPOINT transitions
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
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
- [ ] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [ ] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [ ] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [ ] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-162 and reviewed disposition.
- [ ] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-84 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-163](task-163%20-%20classify-auto-commit-implicit-ddl-and-engine-isolation-boundaries.md) after this child is merged.
