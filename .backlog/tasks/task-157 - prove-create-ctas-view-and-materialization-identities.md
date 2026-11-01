---
id: TASK-157
title: Prove CREATE CTAS VIEW and materialization identities
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-01'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-77
  - TASK-78
  - TASK-79
references:
  - TASK-83
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-83 child 1/4.** Implement prove create ctas view and materialization identities in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-83.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `83-C157-P`, `83-C157-N`.
- **Named mandatory/conditional variants:** CREATE TABLE, CTAS, REPLACE; VIEW/MATERIALIZED VIEW; schema-qualified IDs.
- **Numerical/row-state oracle:** source [1,2] CTAS -> table [1,2]; VIEW remains logical dependency, materialized snapshots frozen at creation.
- **Typed conditional boundary:** No confusing logical view reads with stored physical producer rows. Owner: TASK-157.
- **Original TASK-83 criteria:** #1, #3, #4, #5, #6.

Active frozen version 2 inventory (v1 retained in `docs/task-157-acceptance-freeze-v1.json`): [docs/task-157-acceptance-freeze.json](../../docs/task-157-acceptance-freeze.json), registered in `docs/coverage-manifest.json`. Signoff and required oracle cells remain blocked until verified.

## Verification progress

All nine frozen cases, 117 parser cells, 27 SQL/dbt/ODCS evidence cells and full source/target DuckDB oracles are verified on implementation head `afdf0a49d05304f8cb35208ba98082f57db29251` by [CI run 41348762402](https://github.com/phdah/sql-semantic-protocol/actions/runs/41348762402), with all nine jobs successful. Matt `code-review` second-pass standards/spec review is recorded on PR #178 as review #5730208619 (same GitHub account, separate reviewer pass). Findings on unsupported transactions, vendor table options, MV replacement and complete source population evidence were fixed and regression-tested. All five child acceptance criteria are checked. This signoff changes task/audit/coverage metadata only; the resulting PR head still requires green CI before merging. TASK-83 parent remains open for TASK-158..160 and independent parent closure.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-157 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-83 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-158](task-158%20-%20prove-alter-add-drop-rename-type-column-transitions.md) after this child is merged.
