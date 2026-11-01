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

The [Matt two-axis review COMMENT](https://github.com/phdah/sql-semantic-protocol/pull/175#pullrequestreview-5728876718) reports **no blocking implementation findings** against AGENTS.md/CONTRIBUTING.md or the bounded task specification. The frozen v1-v3 versions are immutable. The active v4 matrix adds exactly the separately reviewed vendor/parser and ordered-replay adapter evidence; it does not expand the production protocol format or invent native vendor/session claims.

## Signoff decision (2026-11-01)

**All five bounded child acceptance criteria have executable and 9/9 green implementation-head verification.** They are marked checked in the task file, and `docs/coverage-manifest.json` records the verified case, parser, adapter, replay and CI counts.

**TASK-154 remains In Progress.** The above COMMENT was submitted with the PR author's connected GitHub account; it is not a distinct-account independent review. The repository's independent-review gate therefore remains open unless the project owner expressly authorizes an exception. No independent reviewer or owner waiver is claimed. The final metadata-head CI is also a premerge gate. TASK-82 parent original AC, TASK-155 dependency and TASK-91 release are separate. Do not merge TASK-154 or start TASK-155 until the child is formally closed and merged on main.
