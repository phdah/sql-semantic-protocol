# TASK-99: sql-tdg pinned physical-source fixture certification

## Immutable source corpus

The authoritative fixture source is `phdah/sql-tdg` at commit
`90ec0e12a2d5cd2c1294556336fc947e176c9db9`. The copied files under
`tests/fixtures/sql_tdg/` are byte-for-byte snapshots, never generated
equivalents. They must be changed only when the pinned upstream revision is
explicitly advanced and all output oracles are reviewed again.

| Family | Committed upstream fixture | Protocol check |
| --- | --- | --- |
| Multi-parent joined GROUP/HAVING | `advanced_pipeline.sql` | `tests/sql_tdg_physical.rs`: zero DAG, positive/rejected DuckDB |
| Joined boundary | `advanced_boundary.sql` | zero DAG, positive DuckDB |
| UNION ALL, UNION, INTERSECT, EXCEPT, DISTINCT, LIMIT | `set_operations.sql` | zero membership and duplicate/NULL DuckDB |
| Ranking/QUALIFY | `raw_sql/window_ranked.sql` | ranked physical DuckDB and NULL/duplicate exclusion |
| Ordered INSERT SELECT | `raw_sql/insert_workflow.sql` | exact typed before/after row bag and DuckDB |
| Correlated EXISTS | `raw_sql/unsupported_exists.sql` | physical dependency graph and DuckDB nonmembership |
| Typed inclusive interval | `raw_sql/range_sampling.sql` | full exact source count, all 14 dialects, DuckDB negative exclusion |
| dbt source and producer DAG | `dbt_core_project/models/*.sql` | eleven committed models, direct parsing, canonical dbt graph, complete DuckDB output snapshots |
| dbt source population | `dbt_core_project/seeds/*.csv` | complete four-table source initialization with duplicate output in union |
| dbt catalog and declared model identity | `dbt_core_project/models/sources.yml` | compiled manifest and warehouse catalog metadata |

The first three SQL snapshots predate TASK-99 and live directly under `tests/fixtures/sql_tdg/`. The other four raw SQL snapshots live under its `raw_sql/` subdirectory. All file paths are enforced by `tests/coverage_manifest.rs`.

The dbt fixture has 11 exact models: `stg_orders`, `stg_customers`,
`enriched_orders`, `aggregate_summary`, `ranked_orders`,
`subquery_orders`, `unioned_orders`, `derived_orders`,
`final_orders`, `boundary_final_orders`, and
`independent_return_summary`. Source files and seeds are unchanged.

## Exactness boundary

A DuckDB row snapshot proves the result for *that concrete complete source
population*, not constructive feasibility of every requested distribution.
Only canonical `WitnessDirection::Feasible` /
`PhysicalWriteStatePlan::Feasible` cases are source-plan-certified.
`Residual` means source generation must fail closed for that requested goal;
`Impossible` is reserved for a proved contradiction. A successful
example must never promote a residual physical plan to a generic feasible
claim.

- Verified exact classes include closed-world all-empty multi-parent
  terminal goals, source-backed range counts, NULL/duplicate-preserving
  direct source states, and pinned INSERT SELECT mutation state.
- A complete multi-parent positive aggregate, ranked/joined window
  distribution, and correlated subquery population can execute on the
  pinned seeds, but may still be **residual** if the canonical planner does
  not supply a complete physical construction. They are **not** claimed as
  accepted arbitrary generator shapes.
- All terminal outputs and rejected/nonmember cases are independently
  observed in DuckDB, with source rows materialized first. This is an
  oracle comparison, not a substitute for a source-level proof.
- Cross-dialect equivalence for the shared range, INSERT, JOIN, GROUP, set, EXISTS and ROW_NUMBER shapes is enforced by `tests/sql_tdg_dialect_laws.rs`, comparing normalized canonical outcomes for every dialect that parses each SQL form. Across all exposed dialects, the universally shared range SQL must
  parse and produce feasible source-backed count proofs. Constructs with
  genuinely different parser boundaries need the dedicated dialect-law
  matrix in TASK-88; no dialect equivalence is silently inferred.

## Executable gates

Run `cargo test --test sql_tdg_physical` for the pinned raw SQL and
`make dbt-e2e` for the actual committed dbt project, manifest,
catalog and complete DuckDB DAG snapshots. The latter runs
`tests/sql_tdg_pinned_dbt_e2e.rs` after dbt seed, run and docs generate.
The normal `cargo test` does not invoke the ignored dbt integration test,
which depends on the external dbt Core toolchain.

Scripted write/DDL operator witnesses outside the one committed upstream
`insert_workflow.sql` are governed by protocol tests
`tests/physical_write_states.rs`,
`tests/physical_write_adapter_parity.rs` and
`tests/ddl_producers.rs`, with source-independent typed snapshots
introduced in TASK-98. The original sql-tdg source revision does not
contain committed standalone UPDATE/DELETE/MERGE/CTAS scripts.
Their *generator* end-to-end acceptance therefore remains separately
pending TASK-31/36: passing the protocol operator oracles cannot be
reinterpreted as a passing generated multi-step workload.

## Cross-repository ownership and release

TASK-99 checks protocol fixture interpretation and physical evidence;
TASK-100 audits all TASK-68 child proofs; TASK-89 validates the broader
cross-feature oracle; TASK-88 owns dialect variants; TASK-91 is the
release gate. Downstream sql-tdg TASK-24..31/35/36 still needs generated
physical-source files, randomized rejected alternatives and complete
actual-run verification. `docs/coverage-manifest.json` retains the
generator rows marked pending until those independent downstream oracles
are green. Do not merge Release Please PR #79 on the strength of this
protocol-side certification alone.
