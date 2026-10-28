---
id: TASK-147
title: Classify conflict overwrite RETURNING and UPSERT syntax
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-28'
labels: []
milestone: m-3
dependencies:
  - TASK-146
references:
  - TASK-80
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-80 child 3/4.** Implement classify conflict overwrite returning and upsert syntax in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-80.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `80-C147-P`, `80-C147-N`.
- **Named mandatory/conditional variants:** INSERT IGNORE/REPLACE/ON CONFLICT; OVERWRITE; RETURNING; vendor dialects.
- **Numerical/row-state oracle:** Target key 1, source keys [1,2]: deterministic DO NOTHING leaves keys [1,2]; RETURNING inserted key=2.
- **Typed conditional boundary:** Unordered collision resolution and overwrite scope are typed residual without engine evidence. Owner: TASK-147.
- **Original TASK-80 criteria:** #1, #2, #3, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-147 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-80 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-148](task-148%20-%20certify-ordered-insert-scripts-and-adapter-parity.md) after this child is merged.

## Child acceptance signoff (2026-10-28)

TASK-147 finite child AC #1..#5 are verified at implementation commit `515aab7939fa94b5a92247a65402fb351273a0e4` with [9/9 GitHub CI checks](https://github.com/phdah/sql-semantic-protocol/actions/runs/40654802652). The immutable v1 and audited v2 inventories are `docs/task-147-acceptance-freeze-v1.json` and `docs/task-147-acceptance-freeze.json`. Source-complete DuckDB, 182 dialect parser cells, 42 adapter cells and active wire-schema regressions are guarded by public tests. Native vendor runtime branches remain typed Residual with explicit TASK-147 ownership and review disposition. Parent TASK-80 stays open; TASK-148 and independent TASK-80 closure are separate.

Delivery PR: https://github.com/phdah/sql-semantic-protocol/pull/158. The signoff metadata commit must also pass final-head CI before merge.
