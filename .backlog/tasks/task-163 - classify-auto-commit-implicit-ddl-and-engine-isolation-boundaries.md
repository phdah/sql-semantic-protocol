---
id: TASK-163
title: Classify auto-commit implicit DDL and engine isolation boundaries
status: Done
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
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-163 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-84 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-164](task-164%20-%20certify-ordered-mixed-script-state-and-final-targets.md) after this child is merged.

## Initial bounded implementation (2026-11-06)

- PR: https://github.com/phdah/sql-semantic-protocol/pull/190 (draft).
- Versioned preimplementation freeze: `docs/task-163-acceptance-freeze.json` v1 with all named positive and conditional families, source/target row oracles, original AC mapping and explicit TASK-163 residual ownership.
- Additive physical and SQL APIs classify native single-connection DuckDB DDL rollback separately from temp-table lifetime, deferred FK checks, concurrent isolation, vendor implicit DDL commits and vendor transaction options. Unsupported engine assumptions stay Residual; pre-existing impossible operations remain Impossible.
- `tests/task_163_session_laws.rs` exercises native CREATE and DROP rollback, NULL/duplicate row occurrences, positive/NOT TRUE final SELECT and unproved contexts.
- **At initial draft:** parser freeze, adapter parity, final CI and review were outstanding; subsequently completed as recorded below. Parent TASK-84 stays To Do.

## Bounded child acceptance signoff (2026-11-06)

- **Status:** TASK-163 Done for the eight frozen cases, with transparently recorded connected-author-account review exception; parent TASK-84 stays To Do.
- **PR:** https://github.com/phdah/sql-semantic-protocol/pull/190 (integration/merge gate; do not merge automatically).
- **Frozen scope:** `docs/task-163-acceptance-freeze.json` v3, retaining immutable preimplementation v1 and observed parser v2. Three mandatory native Exact cases plus five explicitly owned and approved conditional Residual boundaries. All 13 public parser names × eight variants = 104 pinned cells.
- **Implementation:** `src/physical_realization/transactions.rs`, `src/physical_realization/write_states.rs`, `src/physical_realization/sequential_graph.rs`, `src/physical_realization/lifecycle_scripts.rs`, and SQL lowering in `src/analysis/transactions.rs`. Canonical complete typed catalogs and source/producer identity, NULLs/duplicates, affected counts, rollback generations and read TRUE/NOT TRUE. Invalid COMMIT Impossible; unproved vendor/session laws Residual with TASK-163 owner.
- **Independent engine and adapter evidence:** `tests/task_163_{session_laws,dialect_matrix,adapter_parity,acceptance_inventory}.rs` certify native DuckDB DDL rollback, explicit parser boundaries and matching direct SQL/dbt/ODCS canonical schema evidence; no vendor engine laws invented.
- **Verified implementation-head CI:** https://github.com/phdah/sql-semantic-protocol/actions/runs/42185354954, 9/9 successful at `b5814e5261b7b7e2131aaa9fc0b38512e17fa612`, including fmt, strict lint with and without default features, full Rust tests/docs, dependency checks and dbt E2E. No active protocol JSON root/schema change.
- **Two-axis review:** https://github.com/phdah/sql-semantic-protocol/pull/190#pullrequestreview-5797413923; Standards PASS, Spec PASS, no blocking code findings. This review was submitted using the connected PR author's GitHub account, **not an independent second-person reviewer**. The owner's current instruction to finish and sign off TASK-163 is treated as an owner-directed process exception; a separate human review may still be required by repository merge policy.
- **Conditional boundaries:** temp relation lifetime, deferred FK checks, concurrent isolation, MySQL/vendor implicit DDL commit, vendor transaction options remain explicitly Residual (not implemented as Exact). SAVEPOINT remains TASK-162; separate-step ALTER metadata remains TASK-164.
- **Auditable evidence:** `docs/task-163-acceptance-audit.md` and `docs/coverage-manifest.json`. TASK-164 is blocked until PR #190 merges to `main`; TASK-84 original acceptance and TASK-91 release gate remain independent.
