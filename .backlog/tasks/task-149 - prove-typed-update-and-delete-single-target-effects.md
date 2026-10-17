---
id: TASK-149
title: Prove typed UPDATE and DELETE single-target effects
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-75
  - TASK-79
references:
  - TASK-81
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-81 child 1/4.** Implement prove typed update and delete single-target effects in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-81.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `81-C149-P`, `81-C149-N`.
- **Named mandatory/conditional variants:** UPDATE SET literals/computed/DEFAULT; DELETE WHERE; TRUE/FALSE/UNKNOWN; untouched and RETURNING.
- **Numerical/row-state oracle:** target keys [1,2,NULL], UPDATE WHERE key=1 changes exactly one; DELETE WHERE key=NULL changes zero.
- **Typed conditional boundary:** Use same target before/after bag and preserve unknown predicate cases. Owner: TASK-149.
- **Original TASK-81 criteria:** #1, #2, #3, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [ ] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [ ] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [ ] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-149 and reviewed disposition.
- [ ] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-81 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-150](task-150%20-%20prove-update-from-and-delete-using-joined-mutations.md) after this child is merged.
