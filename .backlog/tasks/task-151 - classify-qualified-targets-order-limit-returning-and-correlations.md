---
id: TASK-151
title: Classify qualified targets ORDER LIMIT RETURNING and correlations
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-30'
labels: []
milestone: m-3
dependencies:
  - TASK-150
references:
  - TASK-81
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-81 child 3/4.** Implement classify qualified targets order limit returning and correlations in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-81.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `81-C151-P`, `81-C151-N`.
- **Named mandatory/conditional variants:** correlated target conditions; RETURNING; vendor ORDER/LIMIT; partition-qualified mutations.
- **Numerical/row-state oracle:** UPDATE two rows and RETURNING key => precisely keys [1,2] when supported; ORDER LIMIT 1 touches one deterministic target.
- **Typed conditional boundary:** Unsupported dialect-specific write order or RETURNING shape remains typed residual. Owner: TASK-151.
- **Original TASK-81 criteria:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-151 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-81 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-152](task-152%20-%20compose-insert-update-delete-and-certify-task-81-states.md) after this child is merged.

## Acceptance signoff (2026-10-30)

All five child ACs pass the versioned v3 fixture matrix (v1/v2 preserved):
five mandatory Exact, two mandatory negative, two reviewed conditional
vendor boundaries. Full DuckDB target/source before-and-after bags, selected
and NOT TRUE row identities, duplicate/NULL frequencies, typed numeric bounds,
RETURNING bags, enforced-key Impossible and missing-source Residual are tested.
The named UPDATE changes two rows and returns exactly keys [1,2].
All 104 dialect parser statuses are pinned (52 canonical, 42 unsupported,
10 parser boundary), plus equivalent SQL/dbt/ODCS metadata routes.
Separate Standards/Spec code-review axes found no remaining in-scope
implementation gaps. All nine implementation CI jobs passed:
https://github.com/phdah/sql-semantic-protocol/actions/runs/41022690530

Child Done in PR #163; **merge remains a separate integration gate**.
TASK-81 stays open; TASK-152 is blocked until PR #163 merges into main.
