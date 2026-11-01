---
id: TASK-155
title: Classify repeated matches UPSERT equivalents and constraint effects
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-01'
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
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-155 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-82 open.
<!-- AC:END -->

## Sequencing

Next: [TASK-156](task-156%20-%20compose-computed-and-sequential-merges-and-certify-task-82.md) after this child is merged.

## Final bounded TASK-155 acceptance signoff (2026-11-01)

The [active v4 acceptance freeze](../../docs/task-155-acceptance-freeze.json) is independently executable, with immutable v1/v2/v3 versions and an explicit v3 FK-deferral proof correction. All five task ACs are signed off for the portable contract by the [acceptance reconciliation](../../docs/task-155-acceptance-audit.md):

- **12 mandatory physical cases**: 5 Exact, 2 Impossible, 5 typed Residual with owner TASK-155, named conditional native-law limitations and no invented poststate.
- **1 exact ordered replay**: ON CONFLICT DO NOTHING then a matched MERGE, preserving the original source and successive complete target states.
- **52 parser/dialect classifications** across the 13 exposed dialect identities; **39 source-independent SQL/dbt/ODCS proof paths** for 12 cases plus the ordered replay.
- Full independent DuckDB source, target and FK reference bags, atomic failure observations, per-occurrence TRUE/NOT TRUE partitions and typed nullable interval/domain frequencies.
- All **9/9** required implementation-head checks passed on `3926cee64d80caffce27468b669e6dac56697ae9`: https://github.com/phdah/sql-semantic-protocol/actions/runs/41331817857
- Matt-style [two-axis Standards/Spec review](https://github.com/phdah/sql-semantic-protocol/pull/176#pullrequestreview-5729364657): zero blocking findings in both axes. It is an **author review**, not a distinct-account independent review. The project owner explicitly instructed completion and signoff in this conversation, so this task records an **owner-directed review exception** rather than falsely claiming distinct-account approval.

**Status: Done** in the implementation PR https://github.com/phdah/sql-semantic-protocol/pull/176; the final metadata-head CI is a remaining PR integration gate and must pass before squash merge. **TASK-82 remains open**, and TASK-156 is blocked until this Done PR merges on `main`. TASK-91/release approval remains separate.
