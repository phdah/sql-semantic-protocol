# TASK-151 implementation audit (in progress)

Task: [TASK-151](../.backlog/tasks/task-151%20-%20classify-qualified-targets-order-limit-returning-and-correlations.md)  
Parent: TASK-81 (independent signoff required)

## Implemented / bounded

- Canonicalize direct, target-qualified `UPDATE/DELETE ... RETURNING` identifiers to the existing target-column identity, using the explicit target alias, or the full target name when unaliased.
- Reject an unmatched or alias-shadowed qualifier, wildcard or expression projection, and alias/unqualified duplicate identity instead of constructing fabricated columns.
- Preserve exact UPDATE postimages, DELETE preimages, duplicate occurrences, SQL UNKNOWN nonselection, affected ordinals and typed exact returned-frequency domains through the existing physical write model.
- Bounded, preimplementation acceptance variants and fixed identities `81-C151-P` and `81-C151-N` are in [freeze v1](task-151-acceptance-freeze.json).

## Standards review (interim two-axis review)

- The interim Standards axis found and fixed a potential alias-shadowing overclaim: an aliased target no longer accepts a reference using its shadowed table name. The freeze's unbound-reference family includes this negative regression. Final independent review remains pending.
- Reuses the existing canonical `WriteOperation::returning_columns` contract and physical evaluator, without new parser-dependent public types, runtime dialect gating or dependency changes.
- No active JSON wire shape change; the output columns retain canonical target names.
- Native DuckDB oracle and negative integration tests are in `tests/task_151_qualified_returning.rs`.

## Specification / remaining gates

This is **not** complete TASK-151 acceptance. Do not mark the child Done or unblock TASK-152.

- AC #1: full typed 13-dialect x 3-adapter acceptance cells and exact matrix ownership remain to be frozen and reviewed.
- AC #2/#3: correlated target predicates, vendor ORDER/LIMIT with deterministic native law, and partition-scoped mutations need independent complete pre/post and rejection witnesses. Cases remain explicitly residual/unverified.
- AC #4: per-dialect parser, dbt compiled catalog and ODCS equivalent evidence paths remain unverified.
- AC #5: full implementation CI and independent final standards/spec review are required before signoff.
- Parent TASK-81 and the release gate retain independent closure obligations.

## Verification

Pending GitHub Actions PR checks. No unexecuted local Rust or DuckDB test is claimed green.
