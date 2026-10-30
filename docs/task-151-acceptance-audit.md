# TASK-151 bounded acceptance audit

**Owner:** TASK-151, child 3/4 of TASK-81. Parent acceptance and TASK-152
remain independently gated by completed, merged child work.

## Inventory and full physical source proof

- Frozen v2: [the task-specific matrix](task-151-acceptance-freeze.json)
  holds 4 required exact cases, 2 mandatory rejection cases and
  2 conditional vendor cases. The preimplementation v1 is retained.
- Exact cases prove qualified UPDATE/DELETE RETURNING and correlated
  EXISTS/NOT EXISTS with a single complete independent physical source.
  Selected and NOT TRUE target prestate ordinals, full target before/after,
  unchanged source, affected counts, duplicate and NULL frequency domains
  are checked against independent expected values.
- A correlated key UPDATE with enforced primary-key collision must be
  Impossible, with a native DuckDB failure and preserved target prestate.
  Missing source population is Residual, never an implicitly empty source.
- Direct SQL, dbt catalog and ODCS schema (when the feature is enabled)
  carry equivalent evidence into the same parser-independent protocol.
  Parser dispositions across 13 dialect identities are pinned independently
  of native vendor semantics once final matrix extraction has been reviewed.
- Conditional release ownership: TASK-151 retains vendor write ORDER/LIMIT
  and partition-qualified mutation as typed Residual/parser boundary where
  native engine version, total order, partition identity and execution law
  are not attested. These are **not** Exact or deferred mandatory gaps.

## Standards review (preliminary)

- Canonical AST conversion stays in `src/analysis/writes.rs`; read-only
  correlated source dependencies and query predicates stay in the existing
  protocol. Physical state construction in
  `src/physical_realization/write_states/operators.rs` does not reparse SQL.
- The previous alias-shadowing overclaim is corrected; RETURNING uses the
  active target alias. No new dependency, lint allow, or active JSON shape.
- No claim is made beyond the bounded one-source row-local EXISTS/NOT EXISTS
  semantics or the corresponding DuckDB oracle.

## Specification signoff gates

- **AC1:** v2 frozen with typed outcomes; pin and verify every parser disposition.
- **AC2:** implementation against complete source/target bag, with fail-closed vendor
  classification; validate exact and impossible through CI.
- **AC3:** full DuckDB before/after/RETURNING, NULL, duplicates, source/target
  identity, enforced primary-key error.
- **AC4:** 13 dialect parser statuses plus direct/dbt/ODCS evidence parity.
- **AC5:** fmt, strict Clippy, all-target Rust tests/docs, no-default-features,
  dbt E2E and protocol/schema snapshots on final head.

TASK-151 remains **In Progress** until those gates are green and independently
reviewed; TASK-81 remains open and TASK-152 cannot start yet.
