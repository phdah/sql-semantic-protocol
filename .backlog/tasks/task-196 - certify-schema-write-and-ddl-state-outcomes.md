---
id: TASK-196
title: Certify schema, DML and DDL state-transition outcomes
status: To Do
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-10'
labels: []
milestone: m-3
dependencies:
  - TASK-195
references:
  - TASK-88
  - TASK-178
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-178 scope-split follow-up; TASK-88 release-blocking child.** **38 original variants:** `schema` (11), `dml` (18), and `ddl` (9), **494 dialect cells**. Includes metadata constraint/type parity and missing/conflicting schemas, INSERT/UPDATE/DELETE/MERGE including conflicts/RETURNING, CREATE/ALTER/DROP/RENAME/TRUNCATE and produced relation/view effects. Original TASK-178 AC #1..#5 and TASK-88 AC #1..#8 remain binding at their mapped owner/final gate; completing this bounded task does not approve unrelated matrix rows, vendor runtime support or protocol 3.0.0. Source of truth: [scope split](../../docs/task-178-scope-split.json). Do not introduce parent-child cycles.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Freeze 38 original variant IDs across 13 parser dialects in a task-specific v1 matrix; independently pin initial physical schemas/rows, poststate by relation, affected and rejected row sets, NULL/default/constraint effects and parser/vendor/adapter exceptions.
- [ ] #2 Using public protocol/lifecycle APIs and independent DuckDB fixtures, verify complete before/after mutation state bags, source/target identity, match multiplicity, branch order, correlated WHERE, generated/default/constraint semantics, RETURNING and schema lifecycle. Require strongest sound typed output/effects, no invented successful terminal for native rejected SQL.
- [ ] #3 Preserve unsupported conflict/upsert/vendor-dependent transaction or DML match semantics as explicit unapproved boundaries for TASK-179/TASK-191, without dropping required core behavior or weakening TASK-88's exactness floor.
- [ ] #4 Verify representable direct SQL/dbt manifest/catalog/ODCS facts at the authoritative adapter seam; map every original case to TASK-197/TASK-180, then run final-head fmt, Clippy, Rust/no-default tests/docs, dbt E2E and pinned sql-tdg tests.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to TASK-197. TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.
