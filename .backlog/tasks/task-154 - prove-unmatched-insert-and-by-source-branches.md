---
id: TASK-154
title: Prove unmatched INSERT and BY SOURCE branches
status: Done
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

## Final implementation and owner sign-off (2026-11-01)

The frozen TASK-154 v4 acceptance inventory (9 Exact, 2 Residual, 1 Impossible; 65 shared plus 26 vendor parser cells; SQL/dbt/ODCS equivalent metadata and ordered replay) is executed by the task-owned acceptance tests. All five bounded acceptance criteria above were independently checked against the task contract and satisfied. The final Matt Standards/Spec review fixed one test-classification defect, leaving zero blocking implementation findings: https://github.com/phdah/sql-semantic-protocol/pull/175#pullrequestreview-5728930928 .

Nine of nine full implementation-head CI jobs passed at `84d8d2b152e2d582491dbec231c3ef8c5078fae1`: https://github.com/phdah/sql-semantic-protocol/actions/runs/41323578451 .

**Sign-off:** The project owner explicitly approved TASK-154 and waived the distinct-account independent-review gate after the completed two-axis code review: https://github.com/phdah/sql-semantic-protocol/pull/175#issuecomment-6560869104 . This is an **owner exception**, not a claimed distinct-account approval. TASK-154 is **Done** in the implementation PR, contingent on final metadata-head CI passing before squash merge. TASK-82's original parent criteria and TASK-91 release signoff remain open. TASK-155 begins only once this Done PR is squash-merged on `main`.
