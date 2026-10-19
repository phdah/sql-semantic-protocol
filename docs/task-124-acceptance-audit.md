# TASK-124 independent aggregate parent certification

## Frozen acceptance scope

The task-owned [freeze](task-124-acceptance-freeze.json) fixes `73-C124-P` and `73-C124-N`: **12 mandatory executable complete-bag cases** and **4 conditional grouping extension cases**. Each mandatory case includes SQL, stable physical source IDs, full closed input bag, expected terminal bag, exact typed output domains (including NULL and inclusive bounds), and positive / NOT TRUE evidence. The frozen cases cover COUNT(*), COUNT(expr), SUM, MIN, MAX, AVG, grouped/global empty sets, nullable and duplicate contributors, COUNT DISTINCT, FILTER, Boolean HAVING including UNKNOWN, joined amplification, rejected HAVING, and the empty joined global aggregate.

The portable mandatory matrix is **12 × 13 = 156** dialect/case classifications. `FILTER` permits an explicit parser boundary outside generic, PostgreSQL and DuckDB, not a semantic residual after a successful parse. All four GROUPING SETS, ROLLUP, CUBE and GROUP BY ALL conditional cases have **52** dialect/case classifications; successful parsing must still return typed `Residual` rather than claim an execution law. Native vendor/session semantics are **not** certified. TASK-124 retains conditional ownership and release disposition remains pending.

The adapter matrix is **12 × 3 = 36** mandatory direct SQL, compiled dbt and ODCS schema cells. Each adapter must produce the same full canonical physical proof with output domains, group row identities and source identities. The model fixture `tests/fixtures/conformance/v1/dbt_core_project/models/aggregate_summary.sql` is separately executed as a compiled, materialized dbt model by `make dbt-e2e`; it must produce the versioned conformance terminal snapshot `north:1:42` from the actual ref dependency, not a hand-substituted pretend compilation.

## Executable evidence

- `tests/task_124_parent_aggregate_oracles.rs` independently executes each full source and terminal bag in DuckDB and compares it against the canonical public physical API; it checks output NULL-inclusive finite domains, lower/upper inclusive interval points, joined selected/rejected source pairs and duplicate-identity Impossible versus missing-source Residual.
- `tests/task_124_aggregate_adapters.rs` analyzes the same frozen mandatory source bags through direct SQL, dbt compiled metadata and ODCS source schema, asserting exact common outcomes.
- `tests/protocol_conformance_dbt_e2e.rs` and `tests/fixtures/conformance/v1/suite.json` are authoritative materialized dbt fixture evidence, also checked by the task's frozen model path and expected snapshot.
- TASK-121, TASK-122 and TASK-123 own the focused child proofs. TASK-124 independently attests their named core, NOT TRUE and conditional boundaries without modifying the canonical semantics or the public JSON contract.

## Original TASK-73 AC review

| Original AC | Frozen mandatory/conditional evidence | Closure requirement |
| --- | --- | --- |
| #1 | Basic six aggregates; DISTINCT, FILTER, multiple aggregates/aliases; Boolean HAVING | All mandatory finite domains and source bags exact |
| #2 | Joined amplified duplicates, upstream WHERE; independent source identity; rejected groups | Exact selected/rejected pairs and contributor groups |
| #3 | Global/grouped empty; four grouping extensions | Mandatory exact, conditional parser/residual owned and reviewed |
| #4 | Positive, FALSE and UNKNOWN existing groups; typed inclusive/null bounds | DuckDB and protocol outcomes agree exactly |
| #5 | Committed `aggregate_summary.sql`, dbt E2E, independent single/join DuckDB cases | Full dbt and physical source/terminal oracle |
| #6 | 13 dialects, three adapters, impossible and residual paths, full CI | All checks green on final head; conditional review |

## Verification and release decision

**Not yet signed off.** Review the final PR-head GitHub Actions execution (including `cargo fmt --check`, Clippy `-D warnings`, full `cargo test`, `cargo doc --no-deps`, no-default-features, schema/golden checks and `make dbt-e2e`). Failed or missing mandatory cells block checking TASK-124's five acceptance criteria. Do not mark TASK-73 Done from this child: its six original criteria require a separate post-merge independent review. TASK-91 and protocol release remain independent gates.
