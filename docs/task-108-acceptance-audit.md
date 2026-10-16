# TASK-108 independent parent acceptance audit (2026-10-17)

This is the finite TASK-108 closure evidence for the *six unchanged original*
TASK-70 acceptance criteria. It neither certifies all protocol v3 capabilities
nor substitutes parser acceptance for vendor execution.

## Frozen evidence and independent gates

- Baseline: `b3510e9cd1bbde03cc5ea9d85d403099874fbeee`.
- TASK-101..107: merged on `main` via PRs #109..#115; all seven task files
  have `Done` and all child AC checked. Their merge commits are frozen in
  [the TASK-108 matrix](task-108-acceptance-freeze.json), not inferred from task status.
- TASK-107: 42 explicit TASK-66 predicate/expression variants (28 exact-required,
  14 reviewed conditional), 546 parser/canonical dialect cells, 14 family owners,
  nine independent closed-world DuckDB goldens, nine compositions and seeds
  `0, 1, 42, 20261016`. The final PR #115 head CI passed (run
  [38975931061](https://github.com/phdah/sql-semantic-protocol/actions/runs/38975931061)).
- The [TASK-107 frozen contract](task-107-acceptance-freeze.json) and its
  [conditional semantic review](task-107-conditional-review.md) own the
  executable variant boundaries. TASK-108 maps these *without weakening*
  their originally frozen mandatory exactness or approval status.
- `tests/task_108_parent_signoff.rs` rejects missing upstream task signoff,
  changed parent AC, unowned family, missing proof owner, unverified dialect
  classification, dropped golden/composition/seed, and parent signoff before
  the final CI gate. Full `cargo test` also reruns the public SQL/dbt/ODCS
  equality tests and **independently executed** DuckDB source/output oracles,
  rather than deriving expectations from protocol analysis itself.

## Original TASK-70 acceptance criteria

| Parent AC | Independently auditable owner and evidence |
| --- | --- |
| #1 Arithmetic, casts, predicates, patterns and relation contexts | TASK-101/102/104/106; `src/boolean_witness/numeric.rs`, `src/analysis/{cast_domains,output,writes}.rs`; `tests/task_101_arithmetic.rs`, `tests/task_102_computed_expressions.rs`, `tests/task_104_pattern_collation_nullsafe.rs`, `tests/task_106_composed_predicates.rs` |
| #2 SQL 3VL, cross-column, alias reversibility | TASK-102/103/106; `src/boolean_witness.rs`, `src/analysis/predicates.rs`, `src/physical_realization/graph.rs`; `tests/task_103_cross_column.rs` |
| #3 Regex, collation, temporal/JSON/vendor boundaries | TASK-104/105; `src/analysis/expressions.rs`, `src/analysis/temporal_literals.rs`; `tests/task_105_temporal_json_vendor.rs`, `docs/task-107-conditional-review.md` |
| #4 Positive/NOT TRUE, NULL, proof strengths, domains and physical sources | TASK-101/103/106/107; `src/domain.rs`, `src/physical_realization/{graph,joins}.rs`; `tests/task_107_projection_proofs.rs` and `tests/task_107_frozen_execution_oracles.rs` |
| #5 Direct SQL/dbt/ODCS, 13-dialect and DuckDB conformance | TASK-107; `tests/task_107_dialect_classifications.rs`, `tests/task_107_full_variant_adapters.rs`, `tests/task_107_expression_oracles.rs` |
| #6 Unit/integration/differential, schema and adapter parity | TASK-101..107; `tests/task_107_acceptance_freeze.rs`, `tests/task_107_frozen_execution_oracles.rs`, `tests/emission_contract.rs`, `schema/protocol.schema.json` |

All explicit family, variant, test, module and adapter references are stored
as machine-readable AC-specific rows in the TASK-108 freeze. Those paths and
tests are checked by `tests/task_108_parent_signoff.rs`.

## Fail-closed scope and independent release gates

The required portable expression subset is exact only with its proven width,
NULL, type, collation, source-population and comparison preconditions.
Unattested regex engines, Unicode/collation and CHAR padding laws, arbitrary
implicit casts, calendar/DST/session behavior, JSON path semantics, and opaque
vendor/UDF functions retain typed residual or unknown outcomes. This is a
**reviewed conditional boundary**, not an assertion that missing mandatory
implementation is allowed to count as residual.

The 14 conditional TASK-107 cells have individually reviewed and named
dispositions in `docs/task-107-conditional-review.md`; owner TASK-88 retains
the global 13-dialect/engine laws. TASK-89 still owns unified cross-feature
and dbt/DML/DDL execution, TASK-90 the final v3 wire contract, and TASK-91
protocol/sql-tdg publication. The latter three independent signoffs cannot
be changed by completing TASK-108.

## CI acceptance

TASK-108 requires **nine passing checks on the final PR HEAD**:
fmt, strict all-features Clippy, tests, docs, no-default-features lint/test/doc,
dependency graph and dbt Core E2E. Protocol snapshots execute in `cargo test`.
The implementation head `fac7bba6834b05f1863c33aee00a33d0431f19c9` passed all nine required checks in [CI run 38977162083](https://github.com/phdah/sql-semantic-protocol/actions/runs/38977162083). Parent/child closure is signed in PR #116; the signoff commit also requires final green CI before merge.
