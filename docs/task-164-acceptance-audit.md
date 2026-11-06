# TASK-164 mixed-script bounded acceptance audit

**Status:** TASK-164 Done on 2026-11-06 for the original five child AC, with the owner-directed author-account review-process exception stated below. **PR:** https://github.com/phdah/sql-semantic-protocol/pull/191. TASK-84 original parent acceptance and TASK-91 release approval are separately **open**. The PR merge is a separate integration gate.

## Frozen scope and evidence provenance

- **Immutable versions:** `docs/task-164-acceptance-freeze-v1.json` through `docs/task-164-acceptance-freeze-v5.json` retain the original fixture IDs `84-C164-P`, `84-C164-N`, the eight original variant identities/SQL programs/classifications, and each evidence enhancement. Active `docs/task-164-acceptance-freeze.json` v6 adds bounded signoff metadata without changing the v5 cases or 598 pinned parser cells.
- **Historical timing exception, disclosed:** Original v1 froze finite cases and core expected values before semantic edits, but did not fix every subsequently required per-step catalog, per-column bound, source occurrence, comparison assumption or adapter classification. v2 corrected an initial CREATE checkpoint representation, v3 pinned 598 parser boundaries, v4 pinned complete per-step schema/bag/count/ordinal evidence, and v5 pinned full typed NULL/value frequency bounds and per-case adapter limitations. These strengthening revisions do **not** retroactively make v1 complete. The owner-directed signoff explicitly retains this procedural deviation rather than claiming strict pre-semantic freeze compliance.
- **Finite mandatory core:** Five Exact cases (ordered mixed INSERT/MERGE/DELETE/SELECT, duplicate/NULL contrast, rollback, repeated idempotent MERGE, ALTER default propagation), one Impossible duplicate CREATE; each exact program has complete named live relation catalogs after every statement, schema columns, physical row occurrences, affected counts, read TRUE/NOT TRUE ordinals, complete terminal INTEGER/NULL frequency domains and inclusive bounds.
- **Conditional cases:** Native DuckDB `SAVEPOINT` remains independently owned TASK-162 Residual (pinned DuckDB unsupported); non-DuckDB vendor/session and implicit DDL laws remain TASK-163 Residual. Neither is relabelled Exact from parser acceptance.

## Canonical behavior and independent runtime verification

`src/physical_realization/transactions.rs` advances scalar defaults only after exact graph certification and a proven in-script CREATE generation. It carries canonical default/constraint metadata across independently parsed SQL units and restores all metadata on ROLLBACK; unproved type conversions stay Residual. `src/bundle/state.rs` preserves numeric integer default types rather than injecting untyped decimal metadata. No new consumer SQL reparse, independent fabricated producer state or active protocol root/schema version change is introduced.

`tests/task_164_mixed_scripts.rs` executes complete programs against native DuckDB and checks *every* source/intermediate/terminal table bag, live table existence, schema columns/types, typed frequency histograms, write counts, affected ordinals, output TRUE/NOT TRUE membership, rollback, negative native DDL failure, and both committed and rolled-back ALTER defaults. `tests/task_164_dialect_matrix.rs` verifies the literal 598-cell parser/canonical matrix across thirteen dialect names. `tests/task_164_adapter_parity.rs` proves only representable direct SQL/dbt compiled manifest+catalog/ODCS metadata equivalence; none of those adapter artifacts attests engine transaction/session execution.

## Original child acceptance criteria

| AC | Verified bounded evidence | Technical result |
| --- | --- | --- |
| #1 Versioned pre-semantics scope | Immutable v1-v5, v6 complete signed evidence and 598 pinned dialect cells; historical original-v1 incompleteness explicitly accepted only through the owner-directed process exception | PASS with disclosed historical exception |
| #2 Canonical semantics and classification | Exact ordered before/after states, typed defaults/constraints, isolated Impossible and owned Residual boundaries, no consumer replay | PASS |
| #3 Source/checkpoint/terminal native execution | Five full Exact DuckDB programs, impossible CREATE and two conditional Residual boundary tests, typed bounds and row-occurrence checks | PASS |
| #4 Dialect and adapter parity | Thirteen pinned dialect classifications per variant (598 statement cells); direct SQL/dbt/ODCS representable canonical metadata parity | PASS |
| #5 Final implementation CI and review | https://github.com/phdah/sql-semantic-protocol/actions/runs/42205848985, 9/9 green at `eaf6ef2aa4607c58bf41b61e9f0288a37c643404`; two-axis author-account review below | PASS with disclosed reviewer exception |

## Review, integration and remaining release boundaries

**Matt Pocock review axes:** Standards PASS, Spec PASS, zero blocking technical findings: https://github.com/phdah/sql-semantic-protocol/pull/191#pullrequestreview-5799127718.

**Review provenance:** The connected GitHub PR author's account submitted a COMMENT review, **not an independent second-human approval**. The user explicitly requested completion and signoff of all TASK-164 criteria in this conversation, consistent with the owner-directed author-account exception recorded for TASK-161..163. `independent_review_verified` remains `false` in the active freeze and coverage manifest, while `two_axis_author_account_review_verified` and `review_process_exception_approved` are true. Additional review may be mandatory under repository branch protection; task technical signoff is not a claim that another person reviewed it.

**CI provenance:** Implementation code and test head `eaf6ef2a` completed all nine jobs: fmt, strict Clippy all/no-default features, complete Rust tests with/without defaults, docs, dependency checks and dbt E2E. Later commits record review, immutable freeze archives, checklist and signoff metadata; their PR-head CI is a separate integration verification. No release tag or merge has been made by this signoff.

**Parent/release gates:** TASK-84 remains To Do until its original six criteria and all merged children are independently verified, and TASK-91 remains separate. SAVEPOINT/vendor conditional law restrictions are retained explicitly.
