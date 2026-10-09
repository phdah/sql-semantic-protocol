# SQL generator release coverage inventory

**Scope decisions approved 2026-10-09**, with addendums on seeded randomized per-terminal rejection, a unified dbt + scripted DML/DDL E2E gate, future-extensible opaque semantics, and exact canonical equivalence across 13 parsing dialects. [Approved requirements](coverage-signoff.md). **TASK-66 is complete as the reviewed inventory and cross-repo task handoff; feature evidence and 3.0.0 release sign-off remain pending under TASK-67..91.** This is the scope inventory for the planned single protocol **3.0.0** release and sql-tdg milestone **m-3**, not a statement that every feature works.

The authoritative, machine-readable source is [`coverage-manifest.json`](coverage-manifest.json). Each `?` and `unverified` status is an **explicit open release-proof requirement**, not unfinished inventory work or an approved capability claim. [`tests/coverage_manifest.rs`](../tests/coverage_manifest.rs) checks each dialect/feature cell, parser fixtures and DuckDB SQL oracles. Adding or extending a dialect or semantic feature requires updating the manifest and corresponding tests.

## Evidence semantics

- **P**: one named fixture successfully parses and returns an analysis for that dialect. This does **not** independently certify that canonical semantics, value domains or exact terminal membership are correct.
- **?**: no verified fixture for this exact dialect/feature pair. It is **not** an exclusion and **not** supported by assertion.
- **Operator-local**: an existing typed local witness may describe an operator, but the upstream physical-source DAG proof, constructive positive/negative cases, and exact cardinality have **not** yet been certified.
- **E**: only DuckDB fixture execution is available in this repository. Even E is one exercised SQL shape, not an engine-version or arbitrary-workload guarantee.
- **Release blocking**: every uncertified in-scope combination remains blocking until resolved by its owner and independently verified. **Conditionally deferred** means opaque or inherently unprovable cases require typed external evidence before future support; it is not a permanent feature-family exclusion. Safely bounded variants remain release-blocking. Parser and semantic proof for all claimed supported dialect/variant pairs remains mandatory before 3.0.0 sign-off.

Dialect names here are **inventory entries only**, never a production runtime whitelist. The existing `postgres` alias maps to `postgresql`. Engine versions have not been certified for any external vendor; DuckDB fixture tests use the in-process dependency pinned in `Cargo.toml`.

## Dialect by feature, parser evidence only

| Feature | ansi | bigquery | clickhouse | databricks | duckdb | generic | hive | mssql | mysql | postgresql | redshift | snowflake | sqlite | Scope | Protocol owner | sql-tdg owner |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `predicates.integer_ranges` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-70 | TASK-27 |
| `predicates.boolean_logic` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-70 | TASK-27 |
| `predicates.null_truth` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-70 | TASK-26, TASK-27 |
| `predicates.string_pattern` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-70 | TASK-27 |
| `predicates.computed` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-70 | TASK-27 |
| `predicates.temporal` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-70, TASK-79 | TASK-27 |
| `predicates.comparison_environment` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-70, TASK-79 | TASK-27, TASK-33 |
| `joins.inner` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-71 | TASK-25 |
| `joins.outer` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-71 | TASK-25 |
| `joins.semi_anti` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-71 | TASK-25, TASK-26 |
| `joins.non_equi` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-71 | TASK-25, TASK-27 |
| `joins.repeated_source` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-71 | TASK-25 |
| `sets.union_all` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-72, TASK-69 | TASK-24 |
| `sets.union_distinct` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-72, TASK-69 | TASK-24 |
| `sets.intersect` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-72, TASK-69 | TASK-24 |
| `sets.except` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-72, TASK-69 | TASK-24 |
| `sets.alignment` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-72 | TASK-24 |
| `grouping.basic` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-73, TASK-69 | TASK-28 |
| `grouping.advanced` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-73 | TASK-28 |
| `grouping.supergroups` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-73 | TASK-28 |
| `grouping.having` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-73, TASK-68 | TASK-28 |
| `windows.row_number` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-74 | TASK-29 |
| `windows.ranking` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-74 | TASK-29 |
| `windows.frames` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-74 | TASK-29 |
| `windows.qualify` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-74, TASK-68 | TASK-29 |
| `subqueries.exists` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-75 | TASK-26 |
| `subqueries.in` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-75, TASK-69 | TASK-26 |
| `subqueries.scalar_quantified` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-75 | TASK-26 |
| `scopes.cte` | P | P | P | P | P | P | P | P | P | P | P | P | P | Block | TASK-78, TASK-68 | TASK-24, TASK-25, TASK-26, TASK-28, TASK-29 |
| `scopes.recursive_cte` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Conditional | TASK-78 | TASK-36 |
| `scopes.derived_lateral` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-78, TASK-77 | TASK-26 |
| `ordering.sort` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-76 | TASK-29, TASK-30 |
| `ordering.pagination` | ? | ? | ? | ? | ? | P | ? | ? | ? | ? | ? | ? | ? | Block | TASK-76 | TASK-29, TASK-30 |
| `ordering.sampling` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Conditional | TASK-76 | TASK-30 |
| `relations.advanced_sources` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-77 | TASK-25, TASK-26 |
| `types.structural` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-79 | TASK-27, TASK-31 |
| `schema.constraints` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-79, TASK-87 | TASK-27, TASK-31 |
| `schema.metadata_parity` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-87, TASK-79 | TASK-31, TASK-36 |
| `dml.insert` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-80 | TASK-31 |
| `dml.insert_conflicts` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-80, TASK-82 | TASK-31 |
| `dml.update` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-81 | TASK-31 |
| `dml.delete` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-81 | TASK-31 |
| `dml.merge` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-82 | TASK-31 |
| `ddl.producers` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-83, TASK-78 | TASK-31, TASK-36 |
| `ddl.schema_lifecycle` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-83, TASK-79 | TASK-31 |
| `transactions.ordered_effects` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-84 | TASK-31, TASK-36 |
| `outcomes.cardinality` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-85, TASK-69 | TASK-30 |
| `outcomes.distribution` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-85 | TASK-30 |
| `outcomes.classification` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-86, TASK-68 | TASK-35 |
| `outcomes.dag` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-67, TASK-68 | TASK-24, TASK-25, TASK-26, TASK-27, TASK-28, TASK-29, TASK-31, TASK-35 |
| `execution.dialect_equivalence` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-88, TASK-89 | TASK-33, TASK-36 |
| `execution.dbt_fixture` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Block | TASK-89, TASK-91 | TASK-36 |
| `execution.unsupported_opaque` | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | ? | Conditional | TASK-66, TASK-88, TASK-91 | TASK-33, TASK-36 |

**Coverage counts:** 18 parser fixtures (including a known residual); 10 DuckDB SQL oracle cases; 11 recorded downstream dbt models. **Every row** currently has `physical_source_positive = not_end_to_end_proven`, `physical_source_negative = not_end_to_end_proven`, and `output_cardinality = unverified`. This prevents the local witnesses for TASK-58..65 from being mistaken for complete constructive generator plans. The manifest also expands **251 named syntax/semantic variants across 13 dialects (3,263 logical variant cells)** from fail-closed defaults, with only fixture-backed sparse overrides. The matrix above shows **representative parser evidence**, not proof of every variant in its family. Each variant inherits `variant_evidence_defaults` unless an independent fixture justifies an override. The manifest contains existing local witness status, all dialect records, variant evidence and fixture IDs.

## Generator acceptance ownership

| sql-tdg task | Required protocol tasks | Intended coverage |
| --- | --- | --- |
| TASK-24 | 68, 69, 72, 85 | Bag-aware sets, overlaps, NULLs, branch absence, nested/source DAGs |
| TASK-25 | 68, 69, 71 | Outer/semi/anti/non-equi/self joins and composite paths |
| TASK-26 | 68, 75 | Correlated EXISTS, IN/NOT IN, NULL and nested subqueries |
| TASK-27 | 70, 79 | Typed coupled predicates, casts, patterns, computation |
| TASK-28 | 68, 69, 73 | GROUP BY, HAVING, aggregate results, duplicate contributions |
| TASK-29 | 68, 74, 76 | Partition ordering, rank/QUALIFY, ties and frames |
| TASK-30 | 69, 85 | Exact/bounded terminal cardinality and distributions |
| TASK-31 | 79..84 | INSERT/UPDATE/DELETE/MERGE/UPSERT, before/after and DDL |
| TASK-32 | 68, 85, 86 | Isolated scenarios for incompatible terminal outcomes |
| TASK-33 | 66, 88 | Generator dialect evidence and engine/version status |
| TASK-35 | 68, 86 | Rejected classification per terminal output, negative absence |
| TASK-36 | 89, 91 | Pinned candidate, dbt `make all` and terminal SQL oracles |

sql-tdg **TASK-43** pins the protocol release-candidate SHA prior to publication. The final sign-off belongs to protocol **TASK-91**. Do **not** merge Release Please PR #79 until the final fixture and matrix are accepted.

## Committed dbt and scripted SQL workload inventory

The manifest tracks each of the 11 SQL models currently in `sql-tdg/tests/fixtures/dbt_core_project/models/`. This is an explicit fixture snapshot, not a claim that the project passes the current generator. In particular:

- `aggregate_summary` and `independent_return_summary`: grouped counts and MAX; the first also exercises HAVING.
- `ranked_orders`: joined sources feeding ROW_NUMBER and QUALIFY.
- `subquery_orders` and `unioned_orders`: EXISTS and UNION ALL on composed models.
- `stg_orders`, `stg_customers`, `enriched_orders`, `final_orders`, `derived_orders`, `boundary_final_orders`: typed filters, CASE, joins and compositional lineage.

Scripted oracle inputs are separately mapped for INSERT, UPDATE, DELETE, MERGE and CTAS/replace. The local protocol test files demonstrate partial analyses and some DuckDB behavior, but **none** proves that sql-tdg generates a complete prestate/source/afterstate witness. `make all` in sql-tdg's dbt fixture remains the final TASK-36 acceptance gate, with its default 100 matching and 10 rejected rows and no target.

## Triage and remaining executable evidence

All in-scope families above have a concrete downstream and upstream task owner. The release-blocking gaps map to protocol TASK-67..90 and to downstream TASK-24..31/35/36, with task-level proof requirements. Where current implementation is operator-local, the missing proof belongs to **TASK-67** (shared typed witness algebra) and **TASK-68** (physical-source DAG realization). A parser success cannot close either.

The maintainer **approved scope decisions 1–4** on 2026-10-09, with the addendums in [coverage-signoff.md](coverage-signoff.md). Remaining work is engineering and evidence: full per-terminal seeded negative alternatives, mandatory dbt and DML/DDL `make all`, exact canonical equivalence tests for every claimed variant/dialect, explicit typed conditional deferrals, and documentation that DuckDB tests do not establish native vendor-engine certification.

Unparseable SQL is not an implicit capability. Until parser-boundary regression fixtures exist for a dialect-specific variant, its cell remains `?`; semantic unsupported-but-parseable variants must eventually carry a typed residual or failure reason and a negative generator test. TASK-88 owns per-dialect parser boundaries, engine-specific laws, NULL/collation/timezone assumptions and executable oracle attribution.

## Reproducible evidence

`tests/coverage_manifest.rs` uses the manifest as fixture input, checks 18 named parser fixtures and four **exact canonical protocol equivalence** baselines across all 13 dialects (the sole ignored field is source dialect provenance), checks generic unsupported LIMIT diagnostics and Snowflake MINUS parsing, and runs ten DuckDB SQL oracles for feasible, impossible, NULL, duplicate-result, and output-snapshot cases. These tests do **not** certify downstream data generation or adapter parity. TASK-87 owns dbt/catalog/ODCS adapter parity; TASK-89 owns full compiled dbt and cross-operator oracle coverage.

**Required release sequence:** scope decisions are **approved**; land TASK-67..90 implementations and complete parser/canonical equivalence evidence across all supported dialects (with proven fail-closed conditional deferrals), run sql-tdg pinned-candidate integration including default `--rejected 10`, multi-seed negative coverage and mandatory dbt plus scripted DML/DDL `make all`, verify TASK-91, then publish one 3.0.0 release. Future TASK-92 is not required to publish 3.0.0 unless a deferred capability is newly advertised as supported.

## Approved engineering gates (not final release approval)

The maintainer approved [scope decisions 1–4](coverage-signoff.md) on 2026-10-09. The protocol must carry typed per-terminal negative alternatives that downstream can sample with a deterministic seed, across every provably rejectable predicate/column, and verify absence across full SQL execution. The downstream dbt fixture `make all` must invoke a required DuckDB DML/DDL script harness as well as native dbt model DAGs and assert complete source/target results. Unsupported opaque functions, stochastic behaviors, recursion and vendor laws are *future-extensibility cases*, not permanent exclusions; unsupported evidence fails closed today. For semantically equivalent shared SQL, all thirteen supported parsing dialects must emit equivalent canonical protocol outcomes rather than merely parse successfully; executable DuckDB E2E is distinct from unverified native vendor-engine execution.
