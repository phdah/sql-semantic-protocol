# TASK-154 acceptance audit (implementation branch)

**Scope:** TASK-82 child 2/4. The original parent AC #1..#6 remain separate and unsigned.

## Frozen contract

The task-specific active v4 matrix is `docs/task-154-acceptance-freeze.json`, linked from `docs/coverage-manifest.json`. Immutable v1, v2, and v3 histories are retained: v2 corrects the reviewed missing-source boundary so unknown ON partitions never masquerade as NOT TRUE; v3 freezes the mandatory three-step ordered write replay and explicit transaction execution residual. The v4 revision adds 26 explicit vendor boundary parser cells and equivalent SQL/dbt/ODCS three-write replay evidence. It owns 12 full-bag cases (9 mandatory Exact, 2 Residual, 1 Impossible), 65 shared parser/dialect classifications plus 26 vendor boundary cells, source/target SQL TRUE and NOT TRUE partitions, 3 SQL/dbt/ODCS metadata paths, explicit nullable INTEGER bounds/frequencies, target prestate ordinals and atomicity oracles. The independently executed three-write replay proves each target intermediate is carried forward as state, never invented as an independent source.

## Ownership and evidence

- AC #1: `tests/task_154_acceptance_inventory.rs` verifies fixture IDs, finite cells, typed outcomes, dual-direction ON partitions and coverage manifest invariants.
- AC #2: canonical `src/physical_realization/write_states/operators.rs::merge_rows` owns first-applicable MATCHED, NOT MATCHED [BY TARGET] and NOT MATCHED BY SOURCE actions against complete source/target preimages. The implementation reuses the established task-153 interpreter, no second copy of physical state.
- AC #3: `tests/task_154_merge_unmatched.rs` compares complete native DuckDB and canonical poststates, exact per-column domains and NULL frequencies, selected/rejected source and target ordinals, counts, unchanged source and enforced PK impossibility.
- AC #4: `tests/task_154_dialect_adapter.rs` checks each of 13 exposed parser dialects for five shared SQL forms plus two vendor syntax boundaries and the SQL/dbt/ODCS schema-equivalent exact/negative outcomes for all 12 standalone cases and the ordered replay; native vendor runtime laws remain unclaimed.
- AC #5: all nine implementation-head jobs succeeded in [run 41322646368](https://github.com/phdah/sql-semantic-protocol/actions/runs/41322646368) at `35f8561f6598fc333f1437828929ed9edad6c824`: fmt; default/no-default Rust tests; strict Clippy in both configurations; docs in both configurations; dbt Core E2E; and the active protocol/schema regressions through the full test suite.

## Standards and specification review

[Matt final Standards/Spec review](https://github.com/phdah/sql-semantic-protocol/pull/175#pullrequestreview-5728930928) identified and resolved one test classification issue: canonical analysis errors can no longer be silently classified as parser-boundary failures. No blocking implementation or bounded-spec issues remain. Production SQL semantics, protocol shape and dependencies were unchanged. The immutable v1-v3 acceptance histories and active v4 frozen oracle retain their freeze-time values; current task signoff is recorded in the coverage manifest and task file.

## Signoff decision (2026-11-01)

**TASK-154 signed off as Done:** All five bounded child acceptance criteria have executable evidence. All **nine final implementation-head CI jobs** passed on `84d8d2b152e2d582491dbec231c3ef8c5078fae1` ([run 41323578451](https://github.com/phdah/sql-semantic-protocol/actions/runs/41323578451)), covering fmt, both strict Clippy configurations, complete Rust tests/docs, no-default-features, dbt Core E2E, and protocol/schema regressions.

**Independent-review gate:** The PR author and connected GitHub account are the same, so the two-axis COMMENT is not a distinct-account approval. The project owner explicitly approved a **waiver of that gate** after the review and authorized squash merge: [owner signoff](https://github.com/phdah/sql-semantic-protocol/pull/175#issuecomment-6560869104). No separate-account reviewer is falsely claimed.

**Pre-merge gate:** The final signoff/metadata commit must also pass CI before the PR is squash merged. TASK-82 original parent criteria and TASK-91 release remain open; TASK-155 only unblocks after this Done child is merged into `main`.
