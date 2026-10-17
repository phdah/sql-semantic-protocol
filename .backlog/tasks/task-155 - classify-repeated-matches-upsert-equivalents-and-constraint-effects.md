---
id: TASK-155
title: Classify repeated matches UPSERT equivalents and constraint effects
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-154
references:
  - TASK-82
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-82 child 3/4.** Implement classify repeated matches upsert equivalents and constraint effects in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-82.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `82-C155-P`, `82-C155-N`.
- **Named mandatory/conditional variants:** ambiguous many-to-one MERGE; two NULL keys; unique FK; dialect ON CONFLICT.
- **Numerical/row-state oracle:** duplicate source matching target key 1 => runtime-dependent/error classified; null keys do not equal under ordinary ON.
- **Typed conditional boundary:** No claim of first-winner semantics without attested vendor law. Owner: TASK-155.
- **Original TASK-82 criteria:** #1, #2, #3, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [ ] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [ ] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [ ] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-155 and reviewed disposition.
- [ ] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-82 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-156](task-156%20-%20compose-computed-and-sequential-merges-and-certify-task-82.md) after this child is merged.
