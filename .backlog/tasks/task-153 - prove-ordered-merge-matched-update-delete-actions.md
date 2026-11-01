---
id: TASK-153
title: Prove ordered MERGE matched UPDATE DELETE actions
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
  - TASK-70
  - TASK-75
  - TASK-79
  - TASK-80
  - TASK-81
references:
  - TASK-82
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-82 child 1/4.** Implement prove ordered merge matched update delete actions in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-82.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `82-C153-P`, `82-C153-N`.
- **Named mandatory/conditional variants:** MERGE ON unique key; MATCHED UPDATE/DELETE with WHEN predicates; NULL key.
- **Numerical/row-state oracle:** target keys [1,2], source key [1] => UPDATE 1, preserve 2; when DELETE first matched 1 removed.
- **Typed conditional boundary:** First applicable branch semantics and complete target before/after bag. Owner: TASK-153.
- **Original TASK-82 criteria:** #1, #2, #3, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-153 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-82 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-154](task-154%20-%20prove-unmatched-insert-and-by-source-branches.md) after this child is merged.

## Implementation progress (2026-11-01)

Frozen v1 task matrix linked from the coverage manifest before semantics changes. Matched branch UPDATE/DELETE now records exact prestate target occurrence ordinals; an independent DuckDB full-bag oracle covers ordered predicates, NULLs and unsafe matches. The 65-cell parser matrix, eight-case direct/dbt/ODCS parity, ordered three-write replay and nine-job implementation CI are verified. The project owner explicitly approved this final review and authorized marking TASK-153 Done and squash-merging PR #174. This is an owner-authorized exception to the separate-account reviewer gate, not an independent GitHub reviewer approval. TASK-82 remains open.

### Bounded verification (2026-11-01)

- Frozen v3 acceptance: `docs/task-153-acceptance-freeze.json` (v1/v2 immutable history); task-owned inventory and native DuckDB oracle tests.
- Final reviewed implementation head `5002dbe6e358bdcc71b0649be74e99b0d7c7c825` passed all nine CI jobs: https://github.com/phdah/sql-semantic-protocol/actions/runs/41298375190 .
- Standards/spec follow-up review: https://github.com/phdah/sql-semantic-protocol/pull/174#pullrequestreview-5727635140 .
- On 2026-11-01 the project owner explicitly approved this PR for merge in the accompanying ChatGPT conversation and authorized task closure. Separate-account reviewer approval was not obtained; owner authorization substitutes for that procedural gate in this PR, without falsely attesting independent review.
- AC #1..#5 complete; metadata-head CI must pass before the authorized squash merge. Parent TASK-82 remains To Do.
