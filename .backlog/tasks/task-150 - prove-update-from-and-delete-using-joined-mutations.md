---
id: TASK-150
title: Prove UPDATE FROM and DELETE USING joined mutations
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-149
references:
  - TASK-81
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-81 child 2/4.** Implement prove update from and delete using joined mutations in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-81.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `81-C150-P`, `81-C150-N`.
- **Named mandatory/conditional variants:** unique-key UPDATE FROM; DELETE USING; CTE; multiple candidate rows.
- **Numerical/row-state oracle:** target keys [1,2], source key [1] => exactly one modified/deleted target; duplicate matching source is residual or error per law.
- **Typed conditional boundary:** No multiple-match rewrite without unique identity and engine match assumption. Owner: TASK-150.
- **Original TASK-81 criteria:** #1, #2, #3, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [ ] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [ ] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [ ] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-150 and reviewed disposition.
- [ ] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-81 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-151](task-151%20-%20classify-qualified-targets-order-limit-returning-and-correlations.md) after this child is merged.
