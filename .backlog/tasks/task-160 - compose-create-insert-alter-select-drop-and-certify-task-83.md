---
id: TASK-160
title: Compose CREATE INSERT ALTER SELECT DROP and certify TASK-83
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-02'
labels: []
milestone: m-3
dependencies:
  - TASK-159
references:
  - TASK-83
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-83 child 4/4.** Implement compose create insert alter select drop and certify task-83 in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-83.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `83-C160-P`, `83-C160-N`.
- **Named mandatory/conditional variants:** full ordered schema+row lifecycle, catalog identities, adapter parity.
- **Numerical/row-state oracle:** CREATE empty -> INSERT key1 -> ADD b DEFAULT2 -> SELECT (1,2) -> DROP removes table.
- **Typed conditional boundary:** Require complete before/after catalogs and bags at each step, not final count only. Owner: TASK-160.
- **Original TASK-83 criteria:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-160 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-83 open.
<!-- AC:END -->

## Implementation progress (2026-11-02)

- Task-specific frozen v1 inventory: [docs/task-160-acceptance-freeze.json](../../docs/task-160-acceptance-freeze.json), with exact complete-bag CREATE/INSERT/ALTER/SELECT/DROP and negative/conditional boundaries.
- First implementation slice: prove that a read after a witnessed DROP is impossible instead of silently skipping the SELECT. Native DuckDB and independent protocol regression coverage required.
- Completed and independently checked the bounded child matrix: seven cases (five mandatory Exact/Impossible and two conditional Residual), 416 pinned dialect/parser cells, source-complete intermediate SELECT/output and live schema/bag checkpoints, SQL/dbt/ODCS metadata parity, native DuckDB oracles and exact NULL/bounds/rejected-row assertions. Parent TASK-83 remains open.

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
TASK-160 child AC #1..#5 signed off on 2026-11-02 against implementation head `fd79c21efe4710178b6f1205d0a07016d4063df1`.
The seven-case v3 acceptance matrix is fully covered, including five mandatory Exact/Impossible outcomes and two typed conditional Residual boundaries; 416 statement-by-dialect classifications are frozen. Direct SQL, dbt catalog, ODCS metadata and native DuckDB full-bag oracles agree. All 9 jobs succeeded on [CI run 41509894079](https://github.com/phdah/sql-semantic-protocol/actions/runs/41509894079). A separate Standards/Spec Matt code-review pass, recorded as PR #181 review #5742168813, found zero remaining blockers; it was submitted through the PR author's connected GitHub account, not another reviewer identity. The child is Done; merging PR #181 remains an independent integration gate. TASK-83 and TASK-91 are not signed off.
<!-- SECTION:FINAL_SUMMARY:END -->

## Sequencing

Then independently verify all original TASK-83 AC and parent signoff; TASK-91 is a separate gate.
