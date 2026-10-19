# Code map: find the smallest canonical owner

This map is a **navigation index**, not a second semantic specification. For a change, start with the narrowest owner below. Follow the interfaces to adjacent modules only when required, and run the focused test first. The canonical protocol is the Rust model in `src/protocol/`; its serialized contract is `schema/protocol.schema.json`. Do not duplicate a semantic rule in an adapter or emitter.

Validated Arrow-like witness sidecar serialization: `src/emission/typed_physical.rs` owns deterministic physical value JSON; `schema/typed-physical-population.schema.json` documents its separate, non-protocol wire shape; `tests/physical_typed_serialization.rs` verifies exact bytes, bits, JSON NULL and failing proofs. Generated expression dependency ordering and portable function execution remain in `src/physical_realization/write_states/generated.rs`.

Physical typed target append and ordered nested DML feasibility: `src/physical_realization/write_states/typed_populations.rs` checks before/after closed-world nested snapshots, scalar-filtered INSERTs, attested scalar-default omissions, scalar UPDATE/DELETE and preservation of untouched Arrow values; `src/physical_realization/write_states/evaluator.rs` owns shared three-valued scalar evaluation. Text-key/foreign-key binary comparison evidence: `src/relation.rs` owns source law attestation, `src/physical_realization/write_states/constraints.rs` applies it, and `src/emission/document.rs` emits the provenance.

Portable Boolean CHECK AST and three-valued validator: `src/computed_boolean_checks.rs` owns canonical recursive predicate/evidence, `src/analysis/constraints.rs` normalizes SQL DDL and dbt expressions, `src/physical_realization/write_states/boolean_checks.rs` evaluates exact TRUE/FALSE/UNKNOWN, and `src/emission/constraints.rs` emits the JSON contract.

Physical schema and default semantics: `src/relation.rs` owns typed columns, collation and timezone identity; `src/column_sources.rs` owns defaults and typed row-local generated-expression/opaque identity evidence; `src/computed_keys.rs` and `src/computed_checks.rs` own functional key and arithmetic CHECK facts; `src/constraints.rs` merges and validates constraints; `src/physical_realization/write_states/{constraints,check_comparisons,type_validation,operators,typed_values,typed_populations}.rs` owns closed-world physical feasibility, numeric CHECK validation and nested Arrow-like value proof. Adapter paths should only translate evidence, never implement independent proof laws.

ASOF ordering lineage and unsupported ASOF/APPLY diagnostics: `src/analysis/joins.rs`; correlated `LATERAL` derived-row fail-closed boundary: `src/analysis/relations.rs`; public safety regressions: `tests/task_114_temporal_lateral_boundaries.rs`. The ASOF eligibility predicate is preserved in the canonical join condition without claiming that all eligible pairs are selected. Complete bounded nearest-prior and correlated equality source-bag proofs are certified for the portable frozen forms; unmatched ASOF preservation, tied maximal predecessors and native vendor/session/table-function laws fail closed.

TASK-114 finite complete row bags: `src/protocol/relations.rs` owns typed ASOF and correlated `JoinExecution`; `src/analysis/joins.rs` owns AST-derived correlation and physical source-instance identity, and `src/analysis/queries.rs` the parser-certified special-join outer row shape. `src/constructive/types.rs` owns selected/nonselected complete identity pair laws `FiniteJoinLaw` and `TemporalJoinLaw`, `src/physical_realization/join_finite.rs` owns certified signed-integer nearest/range and LATERAL/APPLY source populations, and `src/physical_realization/join_temporal.rs` owns explicitly zone-free temporal complete bags. `src/constructive/cases.rs` validates source bag invariants. `src/emission/constructive.rs` and `schema/protocol.schema.json` own the wire contract. Independent public oracles: `tests/task_114_finite_populations.rs`, `tests/task_114_canonical_frozen.rs`, `tests/task_114_temporal_population.rs`, `tests/task_114_special_adapter_parity.rs`, `tests/task_114_frozen_execution.rs`, `tests/task_114_dialect_matrix.rs`, `tests/task_114_dialect_constructive.rs`.

TASK-115 source-attested finite three-way joins: `src/physical_realization/finite_chains.rs` owns closed-world source-identity, normalized SQL TRUE/NOT TRUE, nullable composite equality, and downstream COUNT(*) bag evaluation. `src/physical_realization/write_states.rs` remains the canonical owner of physical datatype and PK/FK feasibility. `src/physical_realization/joins.rs` and `grouping.rs` retain the separate homogeneous sufficient count construction. The exact frozen eight-case SQL/DuckDB comparison is in `tests/task_115_finite_chain_bags.rs` and `tests/task_115_duckdb_matrix.rs`; `docs/task-115-acceptance-freeze.json` owns source/terminal values and pending adapter and session evidence. This evidence-driven evaluator does not imply that the solver automatically synthesizes those exact bags from cardinality goals.

TASK-119 bounded set row-shaping: `src/analysis/sets.rs` normalizes typed
literal LIMIT/OFFSET and FETCH ONLY with named ORDER BY output positions;
`src/protocol/sets.rs` owns canonical sort keys and row shaping without AST;
`src/physical_realization/set_trees.rs` certifies closed-world integer tuples,
explicit NULL order, total tie-safe sorting and final exact output domains.
Unattested coercion, BY NAME/CORRESPONDING and nested branch LIMIT remain
Residual. Oracles: `tests/task_119_ordered_set_bags.rs`,
`docs/task-119-acceptance-freeze.json`.

TASK-118 finite nested source-bag proof: `src/analysis/sets.rs` certifies the strict,
lossless single-source CTE boundary and retains independent branch semantics;
`src/protocol/sets.rs` owns the recursive operation tree and positional count
laws; `src/physical_realization/set_trees.rs` evaluates complete source snapshots
once per physical relation and proves nested exact membership/absence,
SQL TRUE/FALSE/UNKNOWN branch partitions, typed terminal domains and duplicate
frequencies. The task-owned fixture matrix is
`docs/task-118-acceptance-freeze.json`; independent oracles are in
`tests/task_118_nested_set_bags.rs`, `tests/task_118_dialect_matrix.rs`,
and `tests/task_118_set_adapters.rs`. Unsupported CTE producer shapes,
coercions, order/limits, and unattested scalar expressions remain Residual.

## Starting points

| Change or question | Read first | Follow only if needed | Focused tests |
|---|---|---|---|
| SQL `JOIN`, ON, equality or join kind | `src/analysis/joins.rs` | `src/analysis/relations.rs`, `src/analysis/queries.rs` (row-shape certification), `src/protocol/query.rs`, `src/join_witness.rs`, `src/physical_realization/joins.rs` | `tests/differential_conformance.rs`, `tests/physical_realization/joins.rs`, `tests/protocol_conformance_physical.rs`, `tests/task_106_composed_predicates.rs`, `tests/task_109_composite_join.rs` |
| TASK-113 portable CROSS, USING, NATURAL, OR-on, non-equi and null-safe joins | `src/analysis/joins.rs` (schema-backed NATURAL column intersection) | `src/physical_realization/join_uniform.rs` (complete uniform Boolean pair populations), `src/physical_realization/joins.rs`, `src/constructive/types.rs`, `src/emission/constructive.rs` | `tests/task_113_portable_joins.rs`, `tests/task_113_parser_matrix.rs`, `tests/task_113_join_adapters.rs` |
| CTE, derived table, source alias, table scope | `src/analysis/relations.rs` | `src/analysis/lineage.rs`, `src/analysis/output.rs` (positional output aliases), `src/composition.rs` | `tests/local_relations.rs`, `tests/task_105_temporal_json_vendor.rs` |
| `WHERE`, `HAVING`, `QUALIFY`, SQL truth | `src/analysis/predicates.rs` | `src/analysis/exactness.rs`, `src/boolean_witness.rs`, `src/domain.rs` | `tests/differential_conformance.rs`, `tests/boolean_witness.rs`, `tests/task_103_cross_column.rs`, `tests/task_104_pattern_collation_nullsafe.rs` |
| Catalog-certified finite and interval arithmetic output/write domains | `src/analysis/arithmetic_domains.rs` | `src/analysis/output.rs`, `src/analysis/queries.rs`, `src/analysis/writes.rs` | `tests/task_101_arithmetic.rs`, `tests/advanced_target_validation.rs` |
| Numeric bounds, scalar domains or comparisons | `src/domain.rs` | `src/analysis/domains.rs`, `src/analysis/typing.rs`, `src/protocol/values.rs` | `tests/typed_domains.rs`, `tests/output_domains.rs` |
| Explicit CAST/TRY_CAST/SAFE_CAST literal outcome domains | `src/analysis/cast_domains.rs` | `src/analysis/output.rs` | `tests/output_domains.rs` |
| Gregorian DATE literal identity casts, literal EXTRACT fields, shared calendar validation and ordering | `src/analysis/temporal_literals.rs`, `src/temporal.rs` | `src/analysis/expressions.rs`, `src/analysis/typing.rs`, `src/domain.rs`, `src/physical_realization/write_states/type_validation.rs` | `tests/task_105_temporal_json_vendor.rs`, `tests/typed_domains.rs` |
| Temporal/JSON/nested unsupported AST boundaries and named opaque-function outcomes | `src/analysis/expressions.rs`, `src/analysis/output.rs` | `src/protocol/values.rs`, `src/boolean_witness.rs`, `docs/coverage-manifest.json` (`task_105_expression_evidence`) | `tests/task_105_temporal_json_vendor.rs` |
| Portable exact-decimal literal output arithmetic | `src/analysis/decimal_literals.rs` | `src/analysis/output.rs` | `tests/output_domains.rs` |
| Width-checked scalar integer arithmetic and reversible predicates | `src/boolean_witness/numeric.rs` | `src/boolean_witness.rs`, `src/protocol/functions.rs` | `tests/task_101_arithmetic.rs` |
| CASE/COALESCE/NULLIF output domains, guard refinement, computed comparisons and derived alias witnesses | `src/analysis/output.rs`, `src/boolean_witness.rs` | `src/analysis/arithmetic_domains.rs`, `src/analysis/predicates.rs`, `src/analysis/queries.rs`, `src/domain.rs`, `src/protocol/functions.rs` | `tests/task_102_computed_expressions.rs`, `tests/task_70_predicates.rs` |
| `GROUP BY`, aggregation or grouped physical rows | `src/analysis/aggregation.rs` | `src/group_witness.rs`, `src/physical_realization/grouping.rs`, `src/protocol/aggregation.rs` | `tests/group_witness.rs`, `tests/physical_realization/group_rank_set.rs`, `tests/protocol_conformance_physical.rs` |
| Window/ranking functions | `src/analysis/functions.rs` | `src/window_witness.rs`, `src/physical_realization/ranking.rs`, `src/physical_realization/grouping.rs` | `tests/physical_realization/mixed.rs`, `tests/physical_realization/group_rank_set.rs` |
| `UNION`, `INTERSECT`, `EXCEPT` | `src/analysis/sets.rs` | `src/protocol/sets.rs`, `src/physical_realization/operators.rs`, `src/physical_realization/sets.rs`, `src/physical_realization/set_bags.rs` (TASK-117 exact two-column source bags) | `tests/set_operations.rs`, `tests/physical_realization/nested_sets.rs`, `tests/task_117_set_bags.rs` |
| `INSERT`, `UPDATE`, `DELETE`, `MERGE` | `src/analysis/writes.rs` | `src/analysis/arithmetic_domains.rs`, `src/protocol/writes.rs`, `src/bundle/layers.rs`, `src/emission/statements.rs` | `tests/dml_transformations.rs`, `tests/advanced_target_validation.rs` |
| Ordered closed-world DML target snapshots | `src/physical_realization/write_states.rs` | `src/physical_realization/write_states/{operators,projection,evaluator,constraints}.rs`, `src/protocol/writes.rs`, `src/physical_realization/counts.rs` | `tests/physical_write_states.rs`, `tests/physical_write_adapter_parity.rs` |
| Correlated or nested subquery membership | `src/analysis/subqueries.rs` | `src/subquery_witness.rs`, `src/physical_realization/membership.rs` | `tests/subquery_witness.rs`, `tests/constructive_membership.rs` |
| Output projection, expression, lineage | `src/analysis/output.rs` | `src/analysis/expressions.rs`, `src/analysis/lineage.rs`, `src/protocol/relations.rs` | `tests/output_domains.rs` |
| Physical projection inversion, complete value histograms | `src/physical_realization/distributions.rs` | `src/physical_realization/joint_distributions.rs`, `src/physical_realization/graph.rs`, `src/outcome_goals.rs`, `src/outcome_proofs.rs` | `tests/physical_realization/distributions.rs` |
| SQL DDL constraints | `src/analysis/constraints.rs` | `src/constraints.rs`, `src/protocol/relations.rs` | `tests/dbt_constraints.rs` |
| Multi-query graph, producer/consumer, cycle | `src/bundle/graph_build.rs` | `src/bundle/graph.rs`, `src/bundle/orchestration.rs`, `src/composition.rs` | `tests/composition.rs`, `tests/physical_realization/graph.rs` |
| Transitive composition and column identities | `src/composition.rs` | `src/bundle/composed.rs`, `src/physical_realization/graph.rs` | `tests/composition.rs`, `tests/composition_exactness.rs` |
| Local constructive obligations and strength | `src/constructive/local.rs` | `src/constructive/types.rs`, `src/constructive/cases.rs`, `src/constructive/direction.rs` | `tests/constructive_witness.rs`, `tests/constructive_adapter_parity.rs` |
| Joint physical source proof | `src/physical_realization/composition.rs` (shared operator histogram reconciliation) | `src/physical_realization/joint_counts.rs` (conjunctive source-row and join/group reconciliation), `src/physical_realization/joins.rs` (filtered join populations, overlapping and independent multi-goal combination, join/group shared-key proofs and materialization laws), `src/physical_realization/graph.rs` (certified widening/total affine inversion, identity mapping and constraint guards), `src/boolean_witness.rs` (canonical affine predicate rewrite and physical truth recheck) | `tests/physical_realization/mixed.rs`, `tests/physical_realization/joint_operators.rs`, `tests/physical_realization/joins.rs`, `tests/task_106_composed_predicates.rs` |
| TASK-108 TASK-70 parent acceptance / independent evidence closure | `docs/task-108-acceptance-freeze.json`, `docs/task-108-acceptance-audit.md` | `docs/task-107-acceptance-freeze.json`, `.backlog/tasks/task-70 - solve-expression-and-predicate-variants.md` | `tests/task_108_parent_signoff.rs`, `tests/task_107_frozen_execution_oracles.rs`, `tests/task_107_dialect_classifications.rs` |
| TASK-106 finite acceptance / proof evidence sign-off | `.backlog/tasks/task-106 - compose-computed-predicates-across-relational-graphs.md` (frozen matrix) | `docs/coverage-manifest.json`, `docs/semantics.md` (certified rules and residual boundaries) | `tests/task_106_signoff.rs`, `tests/task_106_composed_predicates.rs`, `tests/physical_write_states.rs`, `tests/physical_write_adapter_parity.rs` |
| Public Rust protocol model | `src/protocol/` (relevant domain module) | `src/lib.rs`, `schema/protocol.schema.json` | `tests/library_api.rs` |
| Protocol JSON output | `src/emission/` (matching output family) | `src/protocol/`, `schema/protocol.schema.json` | `tests/library_api.rs` |
| dbt inputs, compiled SQL and constraints | `src/dbt.rs` | `src/bundle/orchestration.rs`, `src/constraints.rs` | `tests/dbt_manifest.rs`, `tests/dbt_constraints.rs`, `tests/dbt_core_e2e.rs` |
| Data contracts (ODCS) | `src/odcs.rs` | `src/relation.rs`, `src/constraints.rs` | `tests/odcs.rs` |
| CLI inputs and output options | `src/main.rs` | `src/manifest.rs`, `src/emission/document.rs` | `tests/cli.rs` |

## Emission module ownership

- `src/emission/document.rs`: top-level protocol shape and bundle fields; **only here** assemble the document
- `src/emission/graph.rs` and `composition.rs`: DAG nodes/edges, layer and composed semantics
- `src/emission/statements.rs`: query, writes and mutation state
- `src/emission/operator_witnesses.rs`, `constructive.rs`: operator and constructive proof serialization
- `src/emission/aggregation_sets.rs`, `predicates.rs`, `expressions.rs`, `scalars.rs`: feature-specific SQL values
- `src/emission/relations_domains.rs`, `constraints.rs`, `outcomes.rs`: value domains and evidence

All emitters read canonical parser-independent values. To change protocol meaning, update its actual canonical owner and schema (when required), rather than silently changing one JSON encoder.

## How to use this map

1. Identify the affected semantic concept and read its **first owner**.
2. Read adjacent interfaces and the narrowly relevant test file; do not ingest whole modules/directories by default.
3. If a feature crosses ownership boundaries, follow explicit API contracts and keep evidence/provenance with the canonical owner.
4. Run a targeted test (`cargo test --test <integration-test-name>`), then the repository's full verification gate before merge.

If a path or responsibility changes, update this map **in the same PR**. Keep this document short and factual; it should not become a parallel product roadmap.

TASK-71 source-side orientation: `src/physical_realization/joins.rs` orients recovered filtered-join equality endpoints by the canonical `Join::left/right` inputs before constructing preserved-side outer/null-extension obligations. Public proof regression: `tests/physical_realization/joins.rs`.

TASK-110 three-instance common-key INNER chain: `src/physical_realization/joins.rs` owns the jointly controlled physical counts and SQL alias mapping; `src/constructive/types.rs` and `cases.rs` own `JoinChainPopulation` and invariants, `src/emission/constructive.rs` owns serialization, `tests/task_110_join_chains.rs` owns public/DuckDB regression.

TASK-115 typed independent-filter and composite-ON homogeneous chain construction: `src/physical_realization/joins.rs` checks every equality component and attaches source-row TRUE constraints on the same physical leaves; `src/physical_realization/grouping.rs` lifts complete positive chains into one COUNT(*) group. `tests/task_115_join_filter_group.rs`, `tests/task_115_adapter_parity.rs`, `tests/task_115_duckdb_matrix.rs` and `docs/task-115-acceptance-audit.md` distinguish these sufficient homogeneous proofs from the still-missing finite heterogeneous/null/constrained complete-bag laws.
