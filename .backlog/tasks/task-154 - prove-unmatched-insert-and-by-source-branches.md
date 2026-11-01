---
id: TASK-154
title: Prove unmatched INSERT and BY SOURCE branches
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-01'
labels: []
milestone: m-3
dependencies:
  - TASK-153
references:
  - TASK-82
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-82 child 2/4.** Implement prove unmatched insert and by source branches in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-82.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `82-C154-P`, `82-C154-N`.
- **Named mandatory/conditional variants:** NOT MATCHED BY TARGET INSERT; NOT MATCHED BY SOURCE UPDATE/DELETE; branch order.
- **Numerical/row-state oracle:** target keys [1,2], source keys [1,3] => matched 1, source-only 3 inserted, target-only 2 handled BY SOURCE.
- **Typed conditional boundary:** No intermediate target state treated as an independent source row. Owner: TASK-154.
- **Original TASK-82 criteria:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-154 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-82 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-155](task-155%20-%20classify-repeated-matches-upsert-equivalents-and-constraint-effects.md) after this child is merged.

## Implementation progress (2026-11-01)

Frozen the task-owned 12-case exact/negative physical source/target inventory in `docs/task-154-acceptance-freeze.json` (active v4; immutable historical v1/v2/v3 retained) before semantic changes. It defines exact expected before/after bags, NULL and SQL ON TRUE/NOT TRUE partitions, target occurrence ordinals, typed frequency/bound oracles, 65 shared parser cells, 26 conditional vendor parser cells, 3 evidence sources and conditional residual owners. Also froze an independent three-step ordered replay and explicit transaction-law residual. All five bounded task AC have implementation evidence in the active v4 freeze and its executable oracles. Nine of nine implementation-head checks passed at `35f8561f6598fc333f1437828929ed9edad6c824`: https://github.com/phdah/sql-semantic-protocol/actions/runs/41322646368 . Matt Standards/Spec review recorded no blocking implementation findings: https://github.com/phdah/sql-semantic-protocol/pull/175#pullrequestreview-5728876718 . This author-account COMMENT is not a separate-account independent approval. TASK-154 stays In Progress until an independent reviewer approves or the owner explicitly waives that gate; final metadata-head CI is required before merging. TASK-82 remains open.
