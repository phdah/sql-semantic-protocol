---
id: TASK-156
title: Compose computed and sequential MERGEs and certify TASK-82
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-01'
labels: []
milestone: m-3
dependencies:
  - TASK-155
references:
  - TASK-82
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-82 child 4/4.** Implement compose computed and sequential merges and certify task-82 in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-82.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `82-C156-P`, `82-C156-N`.
- **Named mandatory/conditional variants:** computed SET/INSERT; two MERGEs; nested prior state; dialect matrices.
- **Numerical/row-state oracle:** initial target [(1,1)], two MERGEs increment key1 twice => final payload 3, with full transition oracle.
- **Typed conditional boundary:** Positive/rejected partition and end-state proof with complete DuckDB/script snapshots. Owner: TASK-156.
- **Original TASK-82 criteria:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-156 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-82 open.
<!-- AC:END -->

## Sequencing

Then independently verify all original TASK-82 AC and parent signoff; TASK-91 is a separate gate.

## Verification and signoff status (2026-11-01)

- Original child AC #1-#4 are implemented and executable; bounded frozen v4 matrix and independent DuckDB/adapter/parser oracles are in `docs/task-156-acceptance-audit.md`.
- CI passed all nine required jobs at commit `6ecb763b5c2a923359faaf253ae66e61debf4a80`: https://github.com/phdah/sql-semantic-protocol/actions/runs/41337964042
- AC #5 **signed off** after a fresh independent Matt `code-review` standards/spec audit and successful nine-job GitHub Actions CI at implementation head `b39b1704d7868e79fa6303a5714d87c989e64464` (run 41338444281). This independent assessment checked all six frozen cases, parser/adapter evidence and the TASK-156 AC mapping. No blocking code findings remain. The immutable freeze retains historical pending flags; mutable status is recorded here and in `docs/coverage-manifest.json`.
- Keep TASK-82 open; parent AC #1..#6 need a separate independent closure after children have merged into `main`. TASK-156 is Done before merge by repository policy, but does not unblock the parent until this PR is merged.
