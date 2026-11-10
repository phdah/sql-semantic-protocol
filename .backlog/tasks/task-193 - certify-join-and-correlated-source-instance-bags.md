---
id: TASK-193
title: Certify join and correlated source-instance bags across dialects
status: To Do
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-10'
labels: []
milestone: m-3
dependencies:
  - TASK-192
references:
  - TASK-88
  - TASK-178
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-178 scope-split follow-up; TASK-88 release-blocking child.** **24 original variants:** `joins` (23) and `scopes` (1), **312 dialect cells**. Focus on NULL-safe, INNER/OUTER, SEMI/ANTI, theta and repeated-source joins, correlated LATERAL, shared physical input identity and full terminal bag multiplicities. The MSSQL `LATERAL ... ON TRUE` AST boundary remains owned by TASK-179/TASK-191 where native evidence is absent. Original TASK-178 AC #1..#5 and TASK-88 AC #1..#8 remain binding at their mapped owner/final gate; completing this bounded task does not approve unrelated matrix rows, vendor runtime support or protocol 3.0.0. Source of truth: [scope split](../../docs/task-178-scope-split.json). Do not introduce parent-child cycles.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Freeze task-specific v1 evidence for exactly 24 original variant IDs across all 13 parser dialects, with physical source-instance/alias identities and independent positive/rejected/Impossible/Residual source and terminal expectations.
- [ ] #2 Execute public API plus literal DuckDB goldens for complete input bags and multi-source terminal rows, including duplicate cross-products, NULL extension versus physical NULL, repeated aliases, anti/nonmembership and correlation. Enforce the strongest sound output domains, lineage and counts without decomposing a shared physical source into independent witnesses.
- [ ] #3 Assert canonical whole-document equivalence for supported shared SQL AST forms, retaining genuine dialect meaning differences and unexecuted vendors as explicit named boundaries. Specifically keep `88-C177-S152/mssql` unapproved until TASK-179/TASK-191 can resolve the required law.
- [ ] #4 Check source metadata equivalence from direct SQL/dbt/ODCS where it supplies the same fact; deliver cell-level results to TASK-197/TASK-180 and pass final-head repository CI and relevant sql-tdg consumer checks.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to TASK-194. TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.
