---
id: TASK-180
title: Independently certify complete dialect matrix and gate CI
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-19'
labels: []
milestone: m-3
dependencies:
  - TASK-191
references:
  - TASK-88
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-88 original child 5/5, unchanged final independent gate after the expanded child queue.** Independently certify complete dialect matrix and gate CI. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-88 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `88-C180-P`, `88-C180-N`.
- **Finite variants:** all 13 dialect rows and conditionals; SQL/dbt/ODCS; physical outcomes.
- **Independent oracle/verification baseline:** Full parser equivalence CI across TASK-66 frozen variants and all owned conditionals reviewed without false exactness.
- **Conditional/external evidence boundary:** Any unowned conditional or failed mandatory outcome blocks TASK-88 Done and TASK-91. Owner TASK-180; release disposition pending review.
- **Original parent AC:** #1, #2, #3, #4, #5, #6, #7, #8.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-88 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

After this child, including completed/merged TASK-191 boundary dispositions, audit and sign off TASK-88 separately; TASK-91 controls release readiness.

## TASK-178 original AC handoff (2026-11-10)

Before signing off TASK-180, independently validate the complete 163 x 13 mandatory TASK-178 matrix against TASK-192 predicates/types, TASK-193 joins/scopes, TASK-194 sets, TASK-195 windows/grouping/ordering, TASK-196 schema/DML/DDL, TASK-197 cross-adapter parity, TASK-179 session/vendor evidence, and TASK-191 parser/native exceptions. The baseline TASK-178 Done status only certifies portable tests and explicit observations; it does not satisfy final output-domain/physical-bag certification, original parent #1..#8, or protocol 3.0.0 release readiness. All original TASK-180 AC remain unchanged. [Ownership registry](../../docs/task-178-scope-split.json).

## TASK-192 provisional evidence handoff (pending PR #198)

For original TASK-178 predicates/types, consult [TASK-192's 43-case/559-cell expected outcome freeze](../../docs/task-192-acceptance-freeze.json), [40 full DuckDB predicate truth partitions and three non-predicate evidence cases](../../docs/task-192-native-partitions.json), [adapter parity](../../tests/task_192_adapter_parity.rs), [typed domain soundness](../../tests/task_192_typed_soundness.rs), and the [explicit incomplete acceptance audit](../../docs/task-192-acceptance-audit.md). These are upstream reviewer inputs, not an independent final audit. TASK-180 must independently reconcile every mandatory domain/physical/adaptive law after upstream children Done **and merged**, and retain all vendor/TASK-191 conditional boundaries. Keep TASK-180 To Do and TASK-88/TASK-91 blocked.


## TASK-192 later implementation input (PR #198, still unreviewed)

- Review `docs/task-192-original-query-shapes.json` for 43 independent expected outer source, output and WHERE shapes, and `tests/task_192_correlated_exists.rs` for complete original S013 correlated semijoin source occurrences, including physical ID separate from SQL numeric key, NULL and duplicated matches.
- `tests/task_192_adapter_parity.rs` compares the original S013 exact finite source bag across direct/dbt/ODCS. `tests/task_192_nullable_truth.rs` now also verifies public physical TRUE/NOT TRUE witnesses for S010/S011. `tests/task_192_computed_witness.rs` verifies native DuckDB S027 widening at signed BIGINT maximum and native S032 ABS failure at signed minimum. These native results are **not** portable vendor-law attestations.
- The 559 original cell flags are not independently reviewed; TASK-180's **later** complete-matrix signoff remains entirely open.

## TASK-191 bounded handoff (pending PR #205 merge)

The [TASK-191 v2 independently reviewed exception decisions](../../docs/task-191-reviewed-dispositions-v2.json), [v3 TASK-178/179/197 source-fact handoff](../../docs/task-191-downstream-handoff-v3.json), and [v4 bounded signoff](../../docs/task-191-independent-review-v4.json) are inputs to TASK-180. Original 55 parser rejects, 116 Unsupported and 23 missing exact native terminal cases are source-bound classified, **not** promoted into portable mandatory support or external vendor execution. TASK-180 must independently certify the original complete release matrix, implement unresolved mandatory semantics as required, and preserve vendor/session Residual until attested. This parent remains To Do until TASK-191 is Done **and PR #205 merged**.

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-11-19: TASK-191 and PR #205 are merged. Began TASK-180's independent final gate on `feat/task-180-dialect-release-gate`. Frozen 163 original SQL cases x 13 dialects = 2,119 explicit cells in `docs/task-180-acceptance-freeze.json`, grounded in original SQL and TASK-192..197 review evidence. `tests/task_180_independent_dialect_gate.rs` independently reconciles the frozen cells with the original inventory, adapter review and TASK-191 boundary, and replays original DuckDB positive, FALSE, UNKNOWN and rejected complete bags.

Expanded verification in this PR: all 25 independently frozen predicate/source truth bags now execute against DuckDB and 13 public dialects (325 observations); the current 2,119-cell public parser/canonical matrix executes live with the 22 reviewed historical grouping/STRUCT normalizations. `docs/task-180-exception-and-gap-ledger.json` enumerates the 13 original SQL exception families and 105 exceptional original dialect cells. The release-blocking status is intentional, not a test-signoff substitute.

This is **not final acceptance**: the original unsupported/conditional and missing native/vendor claims must be resolved or explicitly reviewed under the release policy. All five task AC remain unchecked pending exact-head CI, full independent typed semantic/physical matrix signoff, adapter/vendor review and pinned consumer evidence. TASK-88 and TASK-91 remain blocked. See `docs/task-180-acceptance-audit.md`.
<!-- SECTION:NOTES:END -->
