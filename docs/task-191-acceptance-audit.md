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

## Reviewed v2/v3 independent exception disposition (2026-11-18)

- [Reviewed v2 ledger](task-191-reviewed-dispositions-v2.json) contains all original 55 SQL/dialect parser errors, 116 full-public-API Unsupported statements and 23 original DuckDB nonterminal cases, each joined to unchanged original SQL, semantic owner, feature scope, typed conservative domain and decision. The initial v1 freeze remains immutable and **was not** retroactively expanded. Every original advertised feature has `scope=release_blocking`, so no mandatory unsupported behavior is misreported as accepted feature support.
- [Task-owned independent review test](../tests/task_191_independent_review.rs) checks all 194 entries, parser masks and exact Unsupported diagnostic category/code/severity/area against the public API. Where SQL cannot parse, `Impossible` means only "this exact original spelling cannot be analyzed by sqlparser 0.58 in this dialect", **never** that the SQL semantics or vendor runtime are impossible.
- [Independent 23-case DuckDB oracles](../tests/task_191_native_original_adjudication.rs) replay 15 negative/invalid original statements against full source populations with stable physical row identities and assert unchanged complete source poststates; five original stochastic samples are checked as occurrence subbags, nonterminating recursion uses only a finite explicit companion, RANDOM is bounded in [0,1), and original EXPLAIN is checked as variable-shape execution without a fixed plan claim. The original native exceptions are *not* reclassified as successful fixed terminal output bags.
- [v3 downstream handoff](task-191-downstream-handoff-v3.json) individually connects all 194 decisions to TASK-178/TASK-179/TASK-197 and explicit TASK-180 release obligations. [The handoff test](../tests/task_191_downstream_handoff.rs) joins those records to the authoritative source-fact and vendor variant inventories. Direct/dbt/catalog/ODCS source-fact parity is not an engine execution claim.
- This is a **review of conservative exception dispositions, not a feature-implementation waiver**. TASK-88/TASK-180/TASK-91 remain release-blocked on any unimplemented mandatory feature, missing typed semantic equivalence, or unexecuted vendor law.

### Verification outstanding

The review and execution claims above require successful tests on the *final* head. Final exact-head protocol CI, pinned sql-tdg consumer CI and a final two-axis review are required before this child can be marked Done. No signoff marker should be committed before those checks are verified.
