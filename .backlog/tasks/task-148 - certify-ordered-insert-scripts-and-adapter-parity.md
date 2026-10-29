---
id: TASK-148
title: Certify ordered INSERT scripts and adapter parity
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-29'
labels: []
milestone: m-3
dependencies:
  - TASK-147
references:
  - TASK-80
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-80 child 4/4.** Implement certify ordered insert scripts and adapter parity in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-80.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `80-C148-P`, `80-C148-N`.
- **Named mandatory/conditional variants:** INSERT after CTE; later SELECT/DDL; direct/dbt/ODCS; 13 dialect parse matrix.
- **Numerical/row-state oracle:** Snapshot before/after two-row insert then SELECT count 2; rejected duplicate constrained key impossible.
- **Typed conditional boundary:** All four insertion families, target domains and final full-bag DuckDB checks required. Owner: TASK-148.
- **Original TASK-80 criteria:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-148 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-80 open.
<!-- AC:END -->

## Sequencing

Then independently verify all original TASK-80 AC and parent signoff; TASK-91 is a separate gate.

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- The frozen TASK-148 v1 matrix is `docs/task-148-acceptance-freeze.json`.
- `tests/task_148_ordered_insert.rs` asserts complete VALUES, SELECT, CTE SELECT and DEFAULT VALUES write states, subsequent CTAS/read behavior, typed domains and full native DuckDB bags, plus PK Impossible and unproved transaction Residual.
- `tests/task_148_adapter_dialect.rs` exercises all 13 parser dialect identities and source-equivalent SQL/dbt/ODCS metadata paths.
- All five acceptance criteria were independently signed off on implementation head `ce584707d2cbb93988c76c190cc66e74ac02cc05` after 9/9 successful checks covering fmt, strict Clippy, full Rust tests/docs, no-default-features, dbt Core E2E, and protocol/schema snapshots.
- The 182 frozen per-statement parser cells, 52 family/dialect cells, 39 vendor-boundary cells, six main cases, five negative contrasts and three conditional residuals are covered. NULL/NOT TRUE source bags and frozen row identities are verified against exact complete physical populations.
- TASK-80 remains open for separate original parent AC review. TASK-91 remains a separate release gate.
<!-- SECTION:NOTES:END -->

## Final Summary

TASK-148's finite ordered INSERT certification is signed off on the verified implementation head `ce584707d2cbb93988c76c190cc66e74ac02cc05` (9/9 CI successful). All mandatory exact, Impossible and Residual cells have executable evidence; unsupported vendor/session boundaries remain non-approved conditional residuals. Merge readiness still requires green CI on the documentation-signoff PR head. TASK-80 remains open.
