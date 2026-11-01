# TASK-155 independent acceptance reconciliation

Scope: **TASK-155**, child 3/4 of TASK-82. This audit does not close TASK-82 or approve protocol v3.

## Frozen ownership and bounded matrix

- Active matrix: [task-155-acceptance-freeze.json](task-155-acceptance-freeze.json) v4.
- Immutable versions: [v1](task-155-acceptance-freeze-v1.json), [v2](task-155-acceptance-freeze-v2.json), [v3](task-155-acceptance-freeze-v3.json). The v2 FK violation was explicitly changed from Impossible to Residual in v3 after tracing existing FK deferrability semantics in `src/physical_realization/write_states/constraints.rs`. This is a reviewed proof correction, not a silent acceptance shortcut.
- 12 mandatory named single-statement cases plus one exact two-step ordered replay:  **5 Exact, 2 Impossible, 5 Residual**. Every Exact case has complete source and target bag, explicit immutable source occurrences, affected counts, and pre/post INTEGER/NULL bound and frequency expectations. The 5 Residual cases have named owner, precise missing execution proof, and explicit pending native release disposition.
- 4 SQL syntax variants × 13 dialect identities = **52 parser/canonical cells**. MERGE matched/unmatched and named ON CONFLICT DO NOTHING are canonically represented when parsed. ON CONFLICT DO UPDATE must not be normalized as DO NOTHING; parser acceptance is not native success.
- (12 single-statement cases + 1 ordered replay) × 3 equivalent metadata paths = **39 adapter proof comparisons**, with runtime ODCS tests gated by the `odcs` Cargo feature. Direct SQL, compiled dbt manifest/catalog and equivalent ODCS-schema metadata are compared using equal physical populations. The referenced parent schema is independently controlled and explicitly supplied on the dbt path; it is not misrepresented as a dbt-declared fact.
- Native DuckDB source and terminal-state oracles cover the portable Exact cases, enforced key contradictions, FK success/failure observation and NULLs. The absence of a portable FK deferral contract prevents promoting native FK error to an unconditional Impossible classification.

## Acceptance criteria reconciliation

| Criterion | Evidence | Disposition |
| --- | --- | --- |
| AC #1, finite versioned inventory | v4 freeze; all three immutable earlier snapshots; task-owned manifest, exhaustive IDs, typed exact domains and controlled NULL truth partitions in `tests/task_155_acceptance_inventory.rs` | Verified on implementation head |
| AC #2, canonical outcomes | Existing `src/physical_realization/write_states/{operators,constraints}.rs` preserves exact matched/unmatched target and source populations. The 12 cases distinguish native-dependent duplicates, UNIQUE NULL ambiguity, FK deferral and unsupported UPSERT from exact/no-op paths | Verified on implementation head |
| AC #3, complete native oracle | `tests/task_155_merge_conflicts.rs` and `tests/task_155_fk_unique.rs` independently execute DuckDB pre/post bag, source occurrence, NULL and affected-row cases; immediate FK failure remains a portable Residual | Verified on implementation head |
| AC #4, dialect and adapter parity | `tests/task_155_dialect_adapter.rs` locks 52 parser/canonical cells and verifies 12 direct/dbt/ODCS case proofs, including ordered conflict/MERGE replay, refusing to infer native execution from AST support | Verified on implementation head |
| AC #5, final-head CI/review and closure | Full GitHub nine-job suite (Rust fmt, strict lint, default/no-default docs and tests, dependency check, dbt Core E2E), final two-axis review and backlog signoff | Verified, metadata-head CI is a merge gate |

## Conditional/fail-closed cases

- `82-C155-N-ambiguous-winner`: multiple source instances match one target. Cannot infer arbitrary winner nor a cross-engine runtime error.
- `82-C155-N-upsert-update`: PostgreSQL-like ON CONFLICT DO UPDATE is not a proven portable MERGE equivalent. Canonical unsupported remains visible.
- `82-C155-N-fk-missing-population`: no complete referenced physical bag, never treated as empty.
- `82-C155-N-fk-violation`: attested FK and complete bags but no portable immediate/commit-time deferral contract; DuckDB failure is tested but not incorrectly generalized. Owner TASK-155, next enabling scope TASK-84/87.
- `82-C155-N-nullable-unique`: NULLS DISTINCT / NOT DISTINCT uniqueness law is engine-specific and requires authoritative native declaration. No exact duplicate-NULL outcome.

All five conditional cases are explicitly owned by TASK-155 and deferred from native release certification, without weakening any portable exact mandatory case.

## Ordered replay closure

`82-C155-P-ordered-conflict-merge` has two exact transitions on a single target physical bag: ON CONFLICT DO NOTHING preserves the prestate, and the following matched MERGE consumes that same state and updates the original target occurrence. Both intermediate terminal DuckDB bags and source immutability are checked in the task-owned public API and native tests. SQL/dbt/ODCS equality uses the same two-step canonical program.

## Release / parent boundaries

TASK-82 still requires TASK-156 and its independent original-AC audit. TASK-91 remains a separate release gate. No code change in this child asserts those approvals.

## Final CI and two-axis review signoff (2026-11-01)

- Final implementation head: `3926cee64d80caffce27468b669e6dac56697ae9`. All **nine** GitHub CI jobs completed successfully: https://github.com/phdah/sql-semantic-protocol/actions/runs/41331817857, including Rust formatting, strict Clippy, tests/docs with and without default features, dependency check and dbt Core E2E.
- Standards review: **0 blocking documented-standard violations**. The core protocol/semantic wire shape and TASK-153/154 residual contract remain unchanged; only test coverage, freeze evidence and explanatory code comments have been added.
- Spec review: **0 unmet mandatory v4 cells**. All original TASK-155 AC #1..#5 are evidenced above with 12 complete physical cases, one ordered replay, 52 parser cells and 39 evidence paths.
- Review evidence: https://github.com/phdah/sql-semantic-protocol/pull/176#pullrequestreview-5729364657. The review is a two-axis **author review**, not a distinct-account independent approval. The repository owner expressly requested implementation and signoff of every task AC; record this as an **owner-directed exception** for TASK-155, without falsely asserting an external reviewer approval.
- Final metadata-head CI remains required **before PR merge**. TASK-82 original AC and TASK-156 work, and TASK-91 release approval, remain independently open.
