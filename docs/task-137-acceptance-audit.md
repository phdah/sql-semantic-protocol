# TASK-137 finite literal table acceptance audit

**Status: acceptance open.** PR #144 is a focused implementation and
validation branch; this audit describes its current semantic proof contract,
not certification of all five criteria or a TASK-77 release.

## Acceptance inventory and versioned fixture corrections

The original [immutable v1](task-137-acceptance-freeze-v1.json) remains
available. [v2](task-137-acceptance-freeze.json) corrects three demonstrable
mistakes without changing fixture IDs: untyped `[]` cannot claim integer
element evidence, empty-result NULL membership is false (though the nullable
*element type* is separate), and generator identities are one-based positions.
Every mandatory case has exact SQL, source-free full input bag, independent
DuckDB oracle query, expected terminal bag, typed finite domains, stable
row identities and an absent-row goal with Impossible classification.

## Public seams and evidence

- `analyze_sql` / `analyze_inputs`: typed source-free VALUES and
  deterministic array table-normalization without consumer-side SQL parsing.
  Unproven `[]`, `[NULL]` and opaque array expressions remain residual.
- `physical_finite_table_plan`: complete source-free row identities, exact
  terminal multiplicities and domains; malformed requested rows are Residual
  rather than false Impossible. Source-column and composed fanout remain
  unproved instead of inventing physical rows.
- `tests/task_137_frozen_matrix.rs`: seven mandatory positive/NULL/duplicate/
  zero-output cases checked against independent DuckDB SQL and all 104
  dialect/variant parse paths. The EXPLODE native engine law is **not**
  inferred from an executable UNNEST equivalence oracle.
- `tests/task_137_adapter_parity.rs`: direct SQL and compiled dbt v12
  sources feed the same canonical model; asserts output bags and exact JSON.
  ODCS contracts carry *physical relation schemas*, not source-free literal
  row generators. These cases have zero physical source columns, so equivalent
  ODCS evidence does not exist. Do not fabricate an ODCS parity cell.

## Acceptance gates still required

| AC | Required final check |
| --- | --- |
| 1 | Independent reconciliation of every corrected frozen cell, parser and adapter classification, nullable and integer bounds |
| 2 | Validate exact vs residual boundaries for typed empty/all-NULL arrays, one-based ordinality, finite duplicate/absence, and unsupported generator/session cases |
| 3 | Passing final-head DuckDB oracles and public identity/domain/negative membership tests for both fixture IDs |
| 4 | Final 13-dialect accepted/boundary matrix, compiled dbt parity, ODCS inapplicability disposition, and native vendor/session EXPLODE limitation reviewed |
| 5 | Green final-head fmt, strict Clippy, Rust tests, docs, no-default, dbt E2E, schema/golden snapshots and two-axis standards/spec code review |

**No TASK-77 parent criterion or protocol 3.0.0 release is signed off here.**
