TASK-146 bounded INSERT SELECT source composition: `src/analysis/writes.rs`
owns parser-side certification of a single nonrecursive row-local CTE,
`src/protocol/writes.rs` owns the canonical normalized producer and consumer,
and `src/physical_realization/write_states/{projection,operators}.rs` owns
the single complete physical input bag and ordered target poststate.
`src/emission/statements.rs` and `schema/protocol.schema.json` describe
the optional CTE stage pair. Independent DuckDB physical oracles live in
`tests/task_146_insert_select.rs` and the independent full-bag oracle is
`tests/task_146_acceptance_matrix.rs`; dialect and adapter parity are
`tests/task_146_dialect_adapter.rs`. The versioned acceptance inventory is
`docs/task-146-acceptance-freeze.json`. TASK-80 parent remains independent.

TASK-145 source-free INSERT constructors: `src/analysis/writes.rs` recognizes
ordinary VALUES and DEFAULT VALUES, reusing the canonical finite literal bags
in `src/analysis/finite_tables.rs`. `src/protocol/writes.rs` owns the typed
source-free default write marker and effect cardinality; `src/physical_realization/write_states/operators.rs`
applies exact literal rows or one authoritative all-default row, preserving
the complete prestate and schema feasibility. `src/emission/statements.rs`
and `schema/protocol.schema.json` own the emitted action and flag. The v2 finite
acceptance inventory is `docs/task-145-acceptance-freeze.json`; the independent
full-bag DuckDB and typed-domain matrix is `tests/task_145_acceptance_matrix.rs`.
`tests/task_145_insert_values.rs` covers generated fields, attested identity
sequences, missing metadata and canonical wire emission;
`tests/task_145_typed_write.rs` covers Arrow-like state feasibility and attested row-local generated scalar fields (but not sequence allocation);
`tests/task_145_precedence.rs` checks ordered writes and transaction residuals;
`tests/task_145_dialect_adapter.rs` checks all 156 parser cells and 36
SQL/dbt-catalog/ODCS-schema equivalence cases. Vendor-only modifiers fail closed
through `tests/task_145_vendor_boundaries.rs`; their portable semantics belong
to TASK-147. Reviewed signoff evidence is `docs/task-145-acceptance-audit.md`.
TASK-80 parent is separately owned by TASK-145..148.

TASK-136 sampling: `src/analysis/sampling.rs` owns sqlparser AST method/quantity/seed normalization, `src/protocol/sampling.rs` holds source-independent typed evidence, and `src/protocol/query.rs` owns sampled source instances. `src/emission/statements.rs` and `schema/protocol.schema.json` own the optional query `sampling` contract. `src/analysis/sets.rs` refuses a sampled source as an independently materializable lossless set branch. `tests/task_136_sampling.rs` verifies typed JSON, schema and sampled DISTINCT safety; `tests/task_136_frozen_matrix.rs` supplies the native DuckDB exact/Impossible complete-bag oracle. `tests/task_136_dialect_matrix.rs` verifies 143 canonical parser/dialect cells; `tests/task_136_adapter_parity.rs` verifies 33 direct SQL/dbt/ODCS cases, and `tests/task_136_dbt_core_e2e.rs` runs a real compiled model. Versioned child signoff is tracked in `docs/task-136-acceptance-freeze.json` and `docs/task-136-acceptance-audit.md`; native vendor sampling laws remain conditional. Parent TASK-76 is not signed off.

TASK-135 composed pagination: `src/analysis/sets.rs` certifies inner/outer CTE pagination and independently ordered set branch leaves; `src/protocol/{query,sets}.rs` preserves both ordered row-shaping stages and each branch's own constraints. `src/physical_realization/page_composition.rs` proves complete original and terminal bags for source-backed CTE pages; `src/physical_realization/set_trees.rs` applies each branch page before set multiplicity, retaining page-rejected identities and typed terminal domains. `src/emission/{statements,aggregation_sets}.rs` and `schema/protocol.schema.json` own the canonical JSON shape. Oracle fixtures live in `tests/task_135_*.rs`, with real dbt Core seed/model under `tests/fixtures/dbt_core_project`; the signed acceptance matrix is `docs/task-135-acceptance-freeze.json`. Implicit NULL order, unproved peers, and unordered producers are residual; TASK-76 parent is separate.

TASK-134 direct page deduplication: `src/protocol/aggregation.rs` owns parser-independent DISTINCT and DISTINCT ON keys; `src/analysis/sets.rs` normalizes bounded direct ORDER/FETCH/TOP with ties; `src/physical_realization/page_bags.rs` proves complete physical DISTINCT bags, exact per-value contributors and full cutoff peer-group membership. DISTINCT ON without a deterministic selected output fails closed unless every candidate projects the same tuple. `src/emission/aggregation_sets.rs`, `schema/protocol.schema.json` and `docs/protocol.md` own the optional wire-level WITH TIES flag. Mandatory, negative, dialect and adapter oracles are in `tests/task_134_{with_ties,frozen_matrix,adapter_parity,dbt_core_e2e}.rs`; `docs/task-134-acceptance-freeze.json` is the owned acceptance inventory. Parent TASK-76 remains separate.

# Code map: find the smallest canonical owner

TASK-140 finite source-free GROUP BY COUNT(*) over literal VALUES/UNNEST/EXPLODE: `src/analysis/finite_tables.rs` normalizes the whole input bag into canonical `FiniteTableKind::Grouped` with contributor indices; `src/physical_realization/finite_tables.rs` proves exact grouped terminal values, typed domains and requested-row impossibility without SQL reparsing. `tests/task_140_finite_group_bags.rs` owns the independent DuckDB oracle. `FiniteTableKind::Joined` retains both complete independent literal bags and exhaustive equality match pairs in `src/protocol/finite_table.rs`; `src/physical_realization/finite_tables.rs` certifies matched/NOT TRUE Cartesian partitions and terminal grouped identities. Compiled dbt models live under `tests/fixtures/dbt_core_project/models/task140_*.sql` and their native warehouse oracle is `tests/task_140_dbt_core_e2e.rs`. Complete dialect/ODCS parity and independent acceptance audit are tracked under `docs/task-140-acceptance-freeze.json`; TASK-77 parent is not certified.

TASK-133 deterministic direct page bounds: `src/analysis/sets.rs` owns shared
AST-to-`SetRowShaping` normalization for direct SELECT and ordered sets;
`src/protocol/query.rs` stores parser-independent row shaping;
`src/physical_realization/page_bags.rs` proves exact requested-row membership,
physical impossibility and full source/terminal bags and
fail-closed tied/null-order boundaries. `src/emission/aggregation_sets.rs`
owns the shared JSON shape; `tests/task_133_page_bags.rs` and `tests/task_133_frozen_matrix.rs` hold
independent DuckDB, 169 dialect cell and 39 adapter cell oracles;
`tests/task_133_dbt_core_e2e.rs` executes the compiled dbt Core model. `docs/task-133-acceptance-freeze.json` tracks remaining
TASK-133 and TASK-76 signoff obligations.


TASK-139 source-free table generators: `src/analysis/finite_tables.rs` is the only supported AST-to-canonical boundary for typed literal `EXPLODE` (SELECT and FROM forms), alongside `UNNEST`. `src/protocol/finite_table.rs` owns the exact typed row bag, and `src/physical_realization/finite_tables.rs` proves complete positive and missing-row membership. Unattested JSON_TABLE, Snowflake FLATTEN, opaque table UDFs, dynamic shape and native engine-specific laws remain non-exact. The verified v3 acceptance inventory is `docs/task-139-acceptance-freeze.json`; exact and conditional public tests are in `tests/task_139_generators.rs`, `tests/task_139_frozen_matrix.rs` and `tests/task_139_adapter_matrix.rs`, plus `tests/task_139_dbt_core_e2e.rs`. TASK-139 child acceptance is verified; TASK-77 parent remains open.

TASK-138 source-free finite wildcard projections, fixed-key SUM PIVOT and named-column UNPIVOT: `src/analysis/finite_tables.rs` validates complete literal VALUES inputs and SQL operator ASTs before forming canonical typed `FiniteTable` rows in `src/protocol/finite_table.rs`; `src/physical_realization/finite_tables.rs` proves complete physical inputs, per-row contributor identities, closed terminal bags and impossible membership. Opaque inputs, dynamic pivots and unproved arithmetic remain Residual. Public tests: `tests/task_138_finite_star_bags.rs` and `tests/task_138_pivot_unpivot.rs`.

TASK-137 source-free literal tables: `src/analysis/finite_tables.rs` normalizes validated VALUES and UNNEST/EXPLODE literal forms into `src/protocol/finite_table.rs`; `src/physical_realization/finite_tables.rs` proves exact identity/multiplicity/domain and absent-row membership from those canonical rows, not SQL reparsing. `src/emission/statements.rs` and `schema/protocol.schema.json` retain the wire contract; `tests/task_137_finite_tables.rs` compares complete DuckDB output bags. Source-column arrays, opaque generators, joined fanout, unproved element types, and vendor/session semantics remain residual; `docs/task-137-acceptance-freeze.json` preserves the unfinished dialect, adapter and CI matrix.


TASK-131 scalar / quantified SQL-bound proofs: `src/protocol/predicates.rs` defines the canonical `QuantifiedSubqueryPredicate`; `src/analysis/predicates.rs` parses ANY/SOME/ALL, `src/emission/predicates.rs` serializes the JSON wire shape, and `schema/protocol.schema.json` validates it. `src/physical_realization/scalar_quantified.rs` proves complete uncorrelated singleton scalar and source-free quantified terminal bags from attested finite physical sources. `tests/task_131_scalar_quantified.rs` exercises 12 frozen mandatory cases x 13 dialect paths with native complete-bag DuckDB oracles, including nested/LATERAL conditional boundaries; `tests/task_131_adapter_parity.rs` covers SQL, compiled dbt and ODCS source evidence. Full cross-engine native quantifier laws and correlated LATERAL/APPLY remain typed conditional residuals.


This map is a **navigation index**, not a second semantic specification. For a change, start with the narrowest owner below. Follow the interfaces to adjacent modules only when required, and run the focused test first. The canonical protocol is the Rust model in `src/protocol/`; its serialized contract is `schema/protocol.schema.json`. Do not duplicate a semantic rule in an adapter or emitter.

Validated Arrow-like witness sidecar serialization: `src/emission/typed_physical.rs` owns deterministic physical value JSON; `schema/typed-physical-population.schema.json` documents its separate, non-protocol wire shape; `tests/physical_typed_serialization.rs` verifies exact bytes, bits, JSON NULL and failing proofs. Generated expression dependency ordering and portable function execution remain in `src/physical_realization/write_states/generated.rs`.

Physical typed target append and ordered nested DML feasibility: `src/physical_realization/write_states/typed_populations.rs` checks before/after closed-world nested snapshots, scalar-filtered INSERTs, attested scalar-default omissions, scalar UPDATE/DELETE and preservation of untouched Arrow values; `src/physical_realization/write_states/evaluator.rs` owns shared three-valued scalar evaluation. Text-key/foreign-key binary comparison evidence: `src/relation.rs` owns source law attestation, `src/physical_realization/write_states/constraints.rs` applies it, and `src/emission/document.rs` emits the provenance.

Portable Boolean CHECK AST and three-valued validator: `src/computed_boolean_checks.rs` owns canonical recursive predicate/evidence, `src/analysis/constraints.rs` normalizes SQL DDL and dbt expressions, `src/physical_realization/write_states/boolean_checks.rs` evaluates exact TRUE/FALSE/UNKNOWN, and `src/emission/constraints.rs` emits the JSON contract.

Physical schema and default semantics: `src/relation.rs` owns typed columns, collation and timezone identity; `src/column_sources.rs` owns defaults and typed row-local generated-expression/opaque identity evidence; `src/computed_keys.rs` and `src/computed_checks.rs` own functional key and arithmetic CHECK facts; `src/constraints.rs` merges and validates constraints; `src/physical_realization/write_states/{constraints,check_comparisons,type_validation,operators,typed_values,typed_populations}.rs` owns closed-world physical feasibility, numeric CHECK validation and nested Arrow-like value proof. Adapter paths should only translate evidence, never implement independent proof laws.

ASOF ordering lineage and unsupported ASOF/APPLY diagnostics: `src/analysis/joins.rs`; correlated `LATERAL` derived-row fail-closed boundary: `src/analysis/relations.rs`; public safety regressions: `tests/task_114_temporal_lateral_boundaries.rs`. The ASOF eligibility predicate is preserved in the canonical join condition without claiming that all eligible pairs are selected. Complete bounded nearest-prior and correlated equality source-bag proofs are certified for the portable frozen forms; unmatched ASOF preservation, tied maximal predecessors and native vendor/session/table-function laws fail closed.

TASK-114 finite complete row bags: `src/protocol/relations.rs` owns typed ASOF and correlated `JoinExecution`; `src/analysis/joins.rs` owns AST-derived correlation and physical source-instance identity, and `src/analysis/queries.rs` the parser-certified special-join outer row shape. `src/constructive/types.rs` owns selected/nonselected complete identity pair laws `FiniteJoinLaw` and `TemporalJoinLaw`, `src/physical_realization/join_finite.rs` owns certified signed-integer nearest/range and LATERAL/APPLY source populations, and `src/physical_realization/join_temporal.rs` owns explicitly zone-free temporal complete bags. `src/constructive/cases.rs` validates source bag invariants. `src/emission/constructive.rs` and `schema/protocol.schema.json` own the wire contract. Independent public oracles: `tests/task_114_finite_populations.rs`, `tests/task_114_canonical_frozen.rs`, `tests/task_114_temporal_population.rs`, `tests/task_114_special_adapter_parity.rs`, `tests/task_114_frozen_execution.rs`, `tests/task_114_dialect_matrix.rs`, `tests/task_114_dialect_constructive.rs`.

TASK-143 finite scalar IN/NOT IN: `src/subquery_witness.rs` owns the
parser-independent scalar member/correlation keys; `src/physical_realization/in_bags.rs`
evaluates complete source-row bags as SQL TRUE/FALSE/UNKNOWN for scalar
and positional integer tuples, retaining candidate identities, selected/rejected
outer identities and finite terminal domains. Unproved nested filters, non-integer
comparison and native engine differences remain explicit Residual.

TASK-129 finite EXISTS/NOT EXISTS: `src/subquery_witness.rs` owns parser-independent operator-local correlations and membership directions, while `src/physical_realization/exists_bags.rs` evaluates entire caller-attested direct source populations and retains both TRUE and NOT TRUE physical outer identities, complete per-row inner matches, terminal bag, and typed output domain. Unsupported shapes fail closed; adapter parity uses canonical facts only. Frozen evidence: `docs/task-129-acceptance-freeze.json`; public DuckDB, dialect, and adapter proofs: `tests/task_129_exists_bags.rs` and `tests/task_129_adapter_parity.rs`.

TASK-132 grouped and joined correlated EXISTS: `src/analysis/subqueries.rs` owns
parser-certified candidate-preserving and grouped/joined existence classifications;
`src/protocol/values.rs` retains these canonical shape facts without conflating
them with IN/scalar candidate multiplicity. `src/physical_realization/exists_bags.rs`
owns exact conjunctive correlation proofs; `exists_joined.rs` owns full
three-source INNER JOIN identity pairs. `exists_producers.rs` evaluates only
closed-world attested physical leaf bags through canonical filter, scalar CASE,
JOIN and correlated EXISTS producer layers and computes complete terminal
integer/text domains and source provenance. `src/bundle/state.rs` attaches
explicit timestamp-zone evidence without guessing adapter defaults. Frozen
matrix: `docs/task-132-acceptance-freeze.json`; source/oracle tests:
`tests/task_132_composite_correlations.rs`,
`tests/task_132_adapter_parity.rs` and
`tests/protocol_conformance_dbt_e2e.rs`. Unsupported SQL forms, ambiguous
aliases, unverified source metadata and non-DuckDB native engine behavior
remain explicitly uncertified rather than extrapolated.

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

TASK-123 joined aggregate physical proof: `src/physical_realization/aggregate_joins.rs` owns complete two-source INNER JOIN/WHERE/GROUP/HAVING bag enumeration, SQL NOT TRUE rejected pairs, exact group contributor identities and terminal scalar domains; `tests/task_123_join_aggregates.rs` owns the independent DuckDB source/terminal oracle and 13-dialect parser classification; `docs/task-123-acceptance-freeze.json` is the task-specific fixed mandatory/conditional matrix. GROUPING SETS, ROLLUP, CUBE and GROUP BY ALL stay typed Residual until their grouping identities and native/session laws are attested.

TASK-122 DISTINCT and FILTER aggregate contributors and full HAVING group truth: `src/physical_realization/aggregate_bags.rs` owns complete typed integer input bags, distinct contributor identity, row-level FILTER three-valued selection and per-group HAVING TRUE/FALSE/UNKNOWN with exact surviving terminal domains. The frozen source/terminal oracle and complete rejected-group witnesses are in `tests/task_122_aggregate_having.rs` and `docs/task-122-acceptance-freeze.json`; unproved scalar, decimal or vendor laws fail closed.

TASK-121 basic grouped/global closed-world aggregate bags: `src/physical_realization/aggregate_bags.rs` certifies complete typed integer source bags through canonical COUNT(*), COUNT(column), SUM, MIN, MAX and exactly integral AVG. It returns exact terminal source identity, multiplicities, NULLs and finite output domains; unproved AVG scale, overflow, HAVING and DISTINCT fail closed. Public DuckDB regression: `tests/task_121_aggregate_bags.rs`; fixture matrix: `docs/task-121-acceptance-freeze.json`.

TASK-125 portable ranking on complete physical bags: `src/physical_realization/rank_bags.rs` evaluates ROW_NUMBER, RANK and DENSE_RANK with peer groups, explicit NULL order, whole-bag source identities and typed terminal domains. Tied ROW_NUMBER identity, implicit NULL ordering and unsupported window shapes fail closed; frozen cases are in `docs/task-125-acceptance-freeze.json` with independent DuckDB proofs in `tests/task_125_rank_bags.rs`. This does not grant exact grouped/joined ranking or native vendor/session execution laws.
TASK-126 complete navigation and bounded ROWS bags: `src/physical_realization/window_bags.rs` owns parser-independent LAG, LEAD, FIRST_VALUE, LAST_VALUE and COUNT(*) ROWS membership over one validated physical source. It reuses canonical signed order, NULL order, full source identity and exact domain helpers from `rank_bags.rs`. Ambiguous ORDER BY peers, implicit/default RANGE, GROUPS, unproved arguments and vendor-specific laws fail closed. Frozen source and terminal oracles: `docs/task-126-acceptance-freeze.json`; public DuckDB/dialect oracles: `tests/task_126_window_bags.rs`. Common aggregate OVER functions are implemented in TASK-189; additional precision and composition obligations remain TASK-189 signoff blockers.
TASK-127 conjunctive direct-source QUALIFY: `src/physical_realization/window_bags.rs` evaluates parser-independent window comparisons against one shared complete physical bag, including hidden QUALIFY-only windows, peer-stable RANK/DENSE_RANK (with underordered ROW_NUMBER remaining Residual), SQL NULL NOT TRUE, and stable selected/rejected source IDs. Independent evidence lives in `tests/task_127_qualify_bags.rs`; the frozen joined/grouped requirements remain open in `docs/task-127-acceptance-freeze.json` until physical source composition is certified.
TASK-189 common aggregate windows: `src/physical_realization/window_bags.rs` reuses source-attested physical identity and deterministic ROWS membership for COUNT(*), COUNT(column), SUM, AVG, MIN and MAX. Checked signed-integer and binary-exact, scale-attested finite DECIMAL arithmetic preserve full source/terminal bags; nonintegral AVG(INT), unproved integer SUM widening, nonbinary fractions, numeric overflow and session-dependent promotion remain Residual. Uniquely ordered mixed ROW_NUMBER/RANK/DENSE_RANK and navigation/aggregate projections share exact ROWS membership; peers remain Residual. Projected integer COUNT/SUM/AVG/MIN/MAX QUALIFY comparisons classify nullable UNKNOWN as NOT TRUE and preserve stable source identities. Frozen v5 oracle inventory: `docs/task-189-acceptance-freeze.json`; public source/DuckDB/dialect and adapter assertions: `tests/task_189_window_aggregates.rs`, `tests/task_189_window_adapters.rs`, `tests/task_189_decimal_windows.rs`, `tests/task_189_decimal_adapters.rs`, `tests/task_189_native_duckdb_matrix.rs`, and `tests/native/test_task_189_sqlite_native.py` (executed in CI). The v5 audit `docs/task-189-acceptance-audit.md` records explicitly unexecuted vendor/session Residual boundaries and final review prerequisites.


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

TASK-128 bounded advanced window physical proofs: `src/physical_realization/window_bags.rs` implements deterministic source-backed NTILE buckets and complete peer-aware RANGE unbounded/CURRENT and bounded GROUPS frame membership, reusing canonical ORDER BY and explicit NULL ordering from `src/physical_realization/rank_bags.rs`. `docs/task-128-acceptance-freeze.json` owns the typed physical/terminal bag oracles and conditional session boundaries; `tests/task_128_advanced_windows.rs` tests the public entry point against independent DuckDB. Native vendor/session matrix, numeric RANGE offsets, and independent TASK-74 parent signoff remain open.

TASK-73 parent aggregate acceptance: `tests/task_73_parent_closure.rs` pins the original AC and the TASK-121..124 and TASK-115 immutable oracles. `tests/task_73_composed_parent.rs` independently evaluates TASK-115's three-source/two-INNER-join WHERE -> projected CTAS stage -> GROUP/HAVING with exact positive/rejected source triplets and typed terminal domains, checking real DuckDB and all 13 dialects; `tests/task_115_finite_adapter_parity.rs` remains the canonical SQL/dbt/ODCS parity oracle. `docs/task-73-acceptance-freeze.json` and `docs/task-73-acceptance-audit.md` own the bounded parent review. Native vendor runtime laws and TASK-91 are separate gates.

TASK-74 independent parent window acceptance: `docs/task-74-acceptance-freeze.json` pins the original six AC against merged TASK-125/126/189/127/128 immutable complete-bag/typed-domain/parser/adapter matrices. `tests/task_74_parent_closure.rs` independently re-executes joined and grouped upstream QUALIFY SQL against native DuckDB, reads complete physical source and terminal bags and checks typed bounds; it also guards all 34 mandatory and 22 reviewed conditional child case identities plus the parent filtered-input matrix, and exercises a separate parent-frozen nested projected-window rank TRUE/NOT TRUE oracle. `docs/task-74-acceptance-audit.md` owns the independent parent scope and conditional vendor/session dispositions. TASK-91 remains a distinct release gate.

TASK-74 upstream filtered-window physical realization: `src/physical_realization/window_filters.rs` compiles a complete canonical WHERE predicate with attested signed-integer/null laws and evaluates SQL TRUE/FALSE/UNKNOWN for each original source identity. `src/physical_realization/window_bags.rs` constructs partitions only from WHERE-TRUE rows and retains WHERE-not-TRUE identities separately from QUALIFY-rejected identities without changing the public proof struct. `docs/task-74-filtered-qualify-freeze.json` freezes original AC #3 mandatory filtered SQL/oracle variants. `tests/task_74_filtered_qualify.rs` verifies exact complete native DuckDB bags, typed domains, physical identities, 13 dialect paths and Residual failures; `tests/task_74_filtered_adapters.rs` verifies SQL/dbt/ODCS parity.
