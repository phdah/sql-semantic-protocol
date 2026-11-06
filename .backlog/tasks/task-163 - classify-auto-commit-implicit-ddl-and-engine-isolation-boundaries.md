---
id: TASK-163
title: Classify auto-commit implicit DDL and engine isolation boundaries
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-06'
labels: []
milestone: m-3
dependencies:
  - TASK-162
references:
  - TASK-84
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-84 child 3/4.** Implement classify auto-commit implicit ddl and engine isolation boundaries in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-84.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `84-C163-P`, `84-C163-N`.
- **Named mandatory/conditional variants:** DDL implicit commits; temp tables; FK deferral; concurrent isolation; dialect transaction laws.
- **Numerical/row-state oracle:** Single-session deterministic DDL transaction snapshots required; unknown concurrency behavior stays residual.
- **Typed conditional boundary:** No claim cross-engine transaction equivalence from parser AST alone. Owner: TASK-163.
- **Original TASK-84 criteria:** #1, #2, #3, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [ ] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [ ] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [ ] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-163 and reviewed disposition.
- [ ] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-84 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-164](task-164%20-%20certify-ordered-mixed-script-state-and-final-targets.md) after this child is merged.

## Initial bounded implementation (2026-11-06)

- PR: https://github.com/phdah/sql-semantic-protocol/pull/190 (draft).
- Versioned preimplementation freeze: `docs/task-163-acceptance-freeze.json` v1 with all named positive and conditional families, source/target row oracles, original AC mapping and explicit TASK-163 residual ownership.
- Additive physical and SQL APIs classify native single-connection DuckDB DDL rollback separately from temp-table lifetime, deferred FK checks, concurrent isolation, vendor implicit DDL commits and vendor transaction options. Unsupported engine assumptions stay Residual; pre-existing impossible operations remain Impossible.
- `tests/task_163_session_laws.rs` exercises native CREATE and DROP rollback, NULL/duplicate row occurrences, positive/NOT TRUE final SELECT and unproved contexts.
- **Not signed off:** required 13-dialect parser freeze, canonical SQL/dbt/ODCS parity, native complete CI, independent review and remaining cells remain open. Parent TASK-84 stays To Do.
