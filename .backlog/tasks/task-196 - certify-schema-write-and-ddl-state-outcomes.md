---
id: TASK-196
title: Certify schema, DML and DDL state-transition outcomes
status: Done
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-16'
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

## Bounded fixture IDs and evidence gates

- **fixture IDs:** `88-C196-P`, `88-C196-N` (positive and rejected/Impossible physical cases).
- **DuckDB / 13 dialect parser seam:** compare independent closed-source/terminal bags, complete emitted typed protocol outcomes and NULL/boundary evidence; never infer other vendor runtime execution.
- **Residual cases:** preserve explicit unapproved semantic gaps, required proof and downstream owner; never promote an unimplemented mandatory core variant to Exact or approved Residual.
- Freeze the task-owned matrix in `docs/coverage-manifest.json` before changing semantic code.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Freeze 38 original variant IDs across 13 parser dialects in a task-specific v1 matrix; independently pin initial physical schemas/rows, poststate by relation, affected and rejected row sets, NULL/default/constraint effects and parser/vendor/adapter exceptions.
- [x] #2 Using public protocol/lifecycle APIs and independent DuckDB fixtures, verify complete before/after mutation state bags, source/target identity, match multiplicity, branch order, correlated WHERE, generated/default/constraint semantics, RETURNING and schema lifecycle. Require strongest sound typed output/effects, no invented successful terminal for native rejected SQL.
- [x] #3 Preserve unsupported conflict/upsert/vendor-dependent transaction or DML match semantics as explicit unapproved boundaries for TASK-179/TASK-191, without dropping required core behavior or weakening TASK-88's exactness floor.
- [x] #4 Verify representable direct SQL/dbt manifest/catalog/ODCS facts at the authoritative adapter seam; map every original case to TASK-197/TASK-180, then run final-head fmt, Clippy, Rust/no-default tests/docs, dbt E2E and pinned sql-tdg tests.
- [x] #5 Complete exact-head rustfmt, strict Clippy, default/no-default Rust tests and docs, dbt E2E, active schema/goldens and relevant pinned sql-tdg consumer checks. Independently review every bounded acceptance cell and record the linked evidence in `docs/coverage-manifest.json`; TASK-88 parent and TASK-91 release remain unapproved until TASK-180 and separate parent signoff.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to TASK-197. TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.

## Implementation progress (2026-11-16)

- v1 frozen original 38-SQL / 494-cell matrix: `docs/task-196-acceptance-freeze.json`.
- Executable source-anchored native replay: `tests/task_196_original_native_states.rs`, using original 16 DML + 18 schema/DDL complete state-oracle cases and two negative DuckDB vendor syntax cases.
- Public parser/unsupported equivalence guards: `tests/task_196_acceptance_matrix.rs`.
- Outstanding ACs and independent review are explicit in `docs/task-196-acceptance-audit.md`; do not mark Done or unblock TASK-197 before every AC and CI gate is verified.

## Bounded acceptance signoff (2026-11-16)

All five task-specific ACs have an independent, executable bounded-source review. The exact original 38 SQL variants × 13 dialects are adjudicated in `docs/task-196-independent-semantic-review.json`: 402 bounded portable cells, 26 explicit unapproved vendor/cascade C cells and 66 unapproved historical parser/unsupported cells, all with downstream owners. The full original DuckDB source/terminal tests, typed physical mutation/DDL proofs, authoritative adapter fact tests, and both positive/NOT TRUE source-occurrence checks passed protocol CI [9/9 on reviewed implementation `697ba496c974d8749f670160d8d0f9a3f05e4e09`](https://github.com/phdah/sql-semantic-protocol/actions/runs/43977550644). The exact reviewed implementation is pinned in the CI-only [sql-tdg compatibility PR #54](https://github.com/phdah/sql-tdg/pull/54), [consumer CI run 43978131493](https://github.com/phdah/sql-tdg/actions/runs/43978131493) (5/5). The status-change commit itself must also pass CI before merge.

All unapproved original boundaries remain unapproved, without inventing vendor runtime success: S198 ON CONFLICT validation precedence, S208 FK cascade actions (TASK-179), REPLACE/OVERWRITE and original parser U/R incl S220 ALTER SET DEFAULT (TASK-179/TASK-191). TASK-197 owns remaining cross-adapter completion; TASK-180/TASK-88/TASK-91 retain whole matrix/parent/release signoff. This scoped Done state **does not** unblock TASK-197 until this PR is merged into `main`.
