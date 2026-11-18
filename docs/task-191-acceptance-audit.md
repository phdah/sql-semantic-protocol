# TASK-191 acceptance audit (implementation in progress)

**Owner:** TASK-191, TASK-88 child 4/5. **Status:** In Progress; no acceptance criteria signed off and no v3 release approval.

## Immutable original exception inventory

The [TASK-191 v1 freeze](task-191-acceptance-freeze.json) is task-owned and references the unchanged original SQL and parser/native evidence in TASK-177. It contains exactly:

| Original boundary axis | Cells / cases | Fixed source | Present disposition |
| --- | ---: | --- | --- |
| SQL + dialect parser rejections | 55 cells, 12 original SQL IDs | `task-177-parser-boundaries.json` | Rejected original grammar; alternative unproven |
| Explicit Unsupported analyzer results | 116 cells, 10 original SQL IDs | `task-177-analysis-boundaries.json` | Fail-closed Unsupported; no exact semantic claim |
| No successful original native output/poststate | 23 original SQL IDs | `task-177-remaining-original-cases.json` | Negative, conditional or nondeterministic; no invented output |

The axes are **not mutually exclusive by original SQL ID**, but parser and Unsupported classifications are disjoint at the SQL/dialect cell level. A native error, a companion dialect spelling and an external vendor/session claim remain three different facts.

`tests/task_191_boundary_disposition.rs` checks the exact row sets against the independently frozen 251-sample parser matrix, physical original-SQL ledger and source fixture IDs; asserts every currently documented parser rejection and Unsupported case through the **public** analyzer; and executes a separately specified positive/rejected-partner DuckDB companion oracle for LEFT SEMI/ANTI. The control has exact source occurrence IDs, duplicates, NULLs and two complete terminal bags; both original LEFT SEMI/ANTI DuckDB statements are independently required to fail, so their companions are never counted as original SQL certifications.

## Acceptance criteria status

| Criterion | Status | Required for signoff |
| --- | --- | --- |
| AC1 | Partial: complete v1 55/116/23 identity freeze | Versioned independently reviewed *decisions*, stable diagnostics, physical evidence for all rows |
| AC2 | Partial: 55 parser cells frozen and public-replayed | Per-cell alternate semantic/native equivalence or reviewed original-syntax limitation |
| AC3 | Partial: 116 Unsupported cases frozen and public-replayed | Independently adjudicated source-bound diagnostic/typed outcome, plus implemented mandatory semantics where required |
| AC4 | Partial: 23 original native gaps frozen; two negative original controls and complete positive/negative companion bags | Independently verify original native evidence by case type (15 conditional/rejected, 5 stochastic, 1 recursion, 1 RANDOM, 1 EXPLAIN) |
| AC5 | Not met | Full per-cell review, adapter reconciliation, final-head fmt, strict Clippy, all Rust tests/docs/no-default-features, dbt E2E, pinned sql-tdg CI and TASK-180 handoff |

## Evidence and safety boundaries

- `tests/task_177_parser_matrix.rs`, `tests/task_177_native_boundary_probes.rs` and `tests/task_178_native_boundary_adjudications.rs` remain earlier source evidence; this PR does not alter their observed snapshots.
- Only the portable parser/analyzer result is tested across 13 dialects. DuckDB executes the local physical source controls, **not** the other 12 native database engines.
- All external runtime/session laws, unimplemented mandatory features, original SQL without a successful terminal and unreviewed conditional dispositions remain **release-blocking**.
- TASK-180 and parent TASK-88 are not ready for signoff. Do not mark TASK-191 Done until the actual final-head CI and independent per-row decisions are complete.

## Matt Pocock skill workflow

Applied `implement` with public-seam regression tests, original-ticket scope, and Conservative/Fail-Closed acceptance. The required `code-review` axes are repository standards and TASK-191 acceptance correctness. No release signoff is inferred from this implementation PR.
