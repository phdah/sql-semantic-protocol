# TASK-80 independent parent acceptance audit

**Status:** In Progress. This is an independent review gate for the six original
TASK-80 acceptance criteria, not a child implementation, merge authorization or
release approval. The TASK-82 queue stays blocked until this parent and TASK-81
are independently signed off and merged.

## Fixed evidence and original-AC crosswalk

The [v1 parent inventory](task-80-parent-acceptance-freeze.json) indexes all
**32 mandatory** and **18 conditional** cases owned by the frozen TASK-145..148
contracts. Each case retains its child-owned SQL, source/target snapshots,
type/domain proof, rejection classification, dialect/adapter evidence and
runtime boundary. The parent index is not new SQL logic and does not
reclassify conditional cases as exact.

| Child | Delivery | Main evidence | Mandatory / conditional |
| --- | --- | --- | --- |
| TASK-145 | [PR #155](https://github.com/phdah/sql-semantic-protocol/pull/155) | `tests/task_145_acceptance_matrix.rs`, `tests/task_145_dialect_adapter.rs` | 9 / 3 |
| TASK-146 | [PR #157](https://github.com/phdah/sql-semantic-protocol/pull/157) | `tests/task_146_acceptance_matrix.rs`, `tests/task_146_dialect_adapter.rs` | 12 / 3 |
| TASK-147 | [PR #158](https://github.com/phdah/sql-semantic-protocol/pull/158) | `tests/task_147_insert_conflict.rs`, `tests/task_147_dialect_matrix.rs` | 5 / 9 |
| TASK-148 | [PR #159](https://github.com/phdah/sql-semantic-protocol/pull/159) | `tests/task_148_ordered_insert.rs`, `tests/task_148_negative_matrix.rs`, `tests/task_148_adapter_dialect.rs` | 6 / 3 |

`tests/task_80_parent_closure.rs` verifies exact inventory parity with the
four child freezes, unique case/fixture IDs, original AC references, explicit
native/conditional proof classification, 13 parser dialects, three metadata
sources and the parent/release signoff separation. It does **not** substitute
for native DuckDB execution or independent semantic review.

| Original AC | Independent parent evidence to examine | Boundary |
| --- | --- | --- |
| #1 INSERT variants | TASK-145 VALUES/DEFAULT/omission, TASK-146 INSERT SELECT, TASK-147 conflict/RETURNING, TASK-148 ordered scripts | Vendor IGNORE/REPLACE and overwrite behavior must remain classified, not guessed |
| #2 Source, target and column effects | TASK-145 exact literal/default target bags, TASK-146 filtered CTE source, TASK-147 conflict row selection, TASK-148 ordered physical states | Unattested defaults/keys are residual |
| #3 Rejected, untouched and cardinality | TASK-145 duplicate/PK cases, TASK-146 CHECK/PK/NULL cases, TASK-147 source conflict, TASK-148 negative PK case | An unsupported conflict law is not a successful zero-row operation |
| #4 Composition and idempotence | TASK-146 CTE, TASK-148 subsequent SELECT/CTAS and transaction residual | Repeated-application/idempotence claims must be reviewed independently |
| #5 Native oracles | Child-owned DuckDB full-bag fixtures for NULL, duplicates, CHECK/PK, zero rows and ordered writes | No native vendor engine was established from parser evidence |
| #6 Contract and adapter parity | Child frozen 13-dialect paths, SQL/dbt/ODCS source evidence and active protocol snapshots | The 13 names are parser identities, not 13 native engine attestations |

## Findings to resolve before parent Done

1. **Mandatory negative boundaries:** The TASK-147 child freeze uses a
   `mandatory_negative` Residual for unattested target keys and Impossible
   for a NULL PK. These intentionally prevent false exactness, but do not
   carry a typed terminal-domain object. Independent review must decide
   whether the original parent requirement is satisfied by the explicit
   nonconstructive error/uncertainty proof, or a versioned child freeze and
   executable oracle require refinement. Do not quietly treat these as
   successful exact outcome cells.
2. **Transaction/ordered state boundary:** TASK-148 deliberately includes a
   `mandatory_residual` for an unproved transaction law. That is a required
   fail-closed negative contrast, not certification of a transaction engine.
   This must remain separate from TASK-84.
3. **Original AC #4:** The complete original criterion includes idempotence
   and repeated application *only when demonstrable*. Check that a claimed
   positive repeatability outcome has a concrete independent source/target
   oracle; otherwise record the explicit unsupported boundary. Merely
   chaining one INSERT with one read is not an idempotence proof.
4. **Evidence freshness:** Verify all four child PRs are merged, child task
   files match the checked-in freeze versions, the active schema/goldens are
   current, and none of the child-level signoff metadata is stale or
   misleading. Parent signoff requires a fresh complete CI run on the final
   audit head and a separate standards/specification review.

## Matt implement and review gates

This PR uses Matt Pocock's `implement` workflow for a focused parent
acceptance task. The observable seam is the public, frozen child inventory
and coverage registry, not SQL AST internals. No production SQL semantics,
public protocol shape, dependencies or dialect rules are changed.

Run the repository's required final checks on this PR head:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo doc --no-deps
cargo clippy --all-targets --no-default-features -- -D warnings
cargo test --no-default-features
cargo doc --no-deps --no-default-features
```

Also require the CI dbt Core E2E and JSON schema/snapshot checks, independent
DuckDB comparison of the child-owned physical oracles and Matt's
`code-review` standards/specification axes. After blocking findings are
resolved and final CI passes, complete a separate explicit parent signoff
decision and update task state in the PR before merge. Until then
`TASK-80: In Progress`, `parent_signed_off=false` and
`release_approved=false` are intentional.
