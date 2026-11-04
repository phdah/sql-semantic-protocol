# TASK-82 independent parent acceptance crosswalk

**Scope:** The original TASK-82 AC #1..#6, independently of the completed
TASK-153..156 child acceptance criteria. This is a verification PR, not an
authorization to merge protocol v3, and is not itself an independent reviewer
sign-off.

## Merged prerequisites

| Child | Merged PR | Frozen contract | Executable evidence |
| --- | --- | --- | --- |
| TASK-153 | [#174](https://github.com/phdah/sql-semantic-protocol/pull/174) | `docs/task-153-acceptance-freeze.json` v3 | `tests/task_153_merge_matched.rs`, `tests/task_153_dialect_adapter.rs` |
| TASK-154 | [#175](https://github.com/phdah/sql-semantic-protocol/pull/175) | `docs/task-154-acceptance-freeze.json` v4 | `tests/task_154_merge_unmatched.rs`, `tests/task_154_dialect_adapter.rs` |
| TASK-155 | [#176](https://github.com/phdah/sql-semantic-protocol/pull/176) | `docs/task-155-acceptance-freeze.json` v4 | `tests/task_155_merge_conflicts.rs`, `tests/task_155_fk_unique.rs`, `tests/task_155_dialect_adapter.rs` |
| TASK-156 | [#177](https://github.com/phdah/sql-semantic-protocol/pull/177) | `docs/task-156-acceptance-freeze.json` v4 | `tests/task_156_sequential_merges.rs`, `tests/task_156_dialect_matrix.rs`, `tests/task_156_adapter_parity.rs` |

Each child is `Done` on `main`. The review exceptions recorded in the
individual child tasks apply to those children only, never to this parent.

## Independent original AC reconciliation

| Parent AC | Evidence to audit at parent boundary |
| --- | --- |
| #1 Ordered branch normalization and dialect UPSERT | TASK-153 matched first-applicable UPDATE/DELETE, TASK-154 unmatched target/source, TASK-155 typed ON CONFLICT boundaries, TASK-156 computed branches. Parser support is not native dialect-equivalence proof. |
| #2 Complete source/target partitions | Child native TRUE/NOT TRUE occurrence partitions, NULL keys, source multiplicity, enforced uniqueness and conditional repeated-match law. The new parent replay checks both source and target membership at **each** step. |
| #3 Initial, final and rejected physical populations | TASK-153/154 negative fixtures and TASK-155 conflicts; TASK-156 multi-write replay. New `82-PARENT-MIXED-P` asserts both complete target populations and untouched shared source after two MERGEs; `82-PARENT-MULTIMATCH-N` keeps duplicate matches Residual. |
| #4 Computed actions and sequential MERGEs | TASK-154 three-way matched/unmatched combination and TASK-156 computed assignments. New parent replay combines computed update, unmatched insertion, by-source deletion, matched deletion and a second computed MERGE. |
| #5 Native and dialect evidence | Child-specific full DuckDB before/after oracles; native two-step parent replay against an independent frozen bag. Existing child matrices enumerate 13 parser dialect identities, not 13 native engines. |
| #6 Tests, docs, schema and adapters | Child acceptance inventories, default/no-default Rust test suites, strict lint, dbt E2E, direct SQL/dbt/ODCS parity and protocol/schema regressions. No new public protocol shape or API is introduced here. |

## Parent-owned finite oracle

`docs/task-82-parent-acceptance-freeze.json` v1 fixes two additional
fixtures before their test implementation:

- `82-PARENT-MIXED-P`: initial target `[(1,10),(2,20),(NULL,8)]`;
  source `[(1,5),(3,30)]`. First MERGE yields
  `[(1,15),(NULL,8),(3,30)]` with one UPDATE, INSERT and DELETE.
  Second MERGE yields `[(NULL,8),(3,40)]` with one UPDATE and DELETE.
  Each step freezes the exact INTEGER/NULL domain, bounds, inclusive
  endpoints, complete source/target TRUE/NOT TRUE counts, and full bags.
- `82-PARENT-MULTIMATCH-N`: two matching source rows for one target,
  which must be `Residual(UnprovedOperation)`, not a fabricated winner.

`tests/task_82_parent_merge.rs` uses the public analyzer and physical write
plan, compares the frozen pre/post states against separately executed DuckDB
statements, and never treats an intermediate target as independently sourced.

## Conditional boundaries and unfinished gates

The parent does **not** certify arbitrary native UPSERT equivalence, MERGE
OUTPUT behavior, duplicate-match winner/error rules, vendor-specific NULL
uniqueness or FK deferral, or transaction/session behavior. TASK-155 owns
the named conditional cases; TASK-84/87 own further enabling evidence.
A parser-accepted statement is never native-engine proof.

This audit is **pending** the latest PR-head CI, a distinct independent
parent-level Standards/Spec review, and explicit acceptance of any remaining
conditional vendor dispositions. No child review waiver is inherited.
Original TASK-82 AC #1..#6 stay unchecked until those gates are satisfied.
TASK-83, TASK-85 and TASK-91 require their own independent closure, and
TASK-161 cannot begin until TASK-82 and TASK-83 are Done and merged.
