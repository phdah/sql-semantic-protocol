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

## Final verification and release decision

**Bounded TASK-124 child signed off, TASK-73 parent and release remain open.** All 12 mandatory frozen complete physical source/output oracles, the 156 mandatory and 52 conditional dialect cells, 36 mandatory exact and 12 conditional fail-closed adapter cells, complete group TRUE/FALSE/UNKNOWN and contributor identities, and the actual materialized `aggregate_summary.sql` dbt output were independently reviewed against source assertions.

**Reviewed semantic CI:** [run 39316409531](https://github.com/phdah/sql-semantic-protocol/actions/runs/39316409531) at `b74b94ea44be9133f8f0b8388340c61cda5e5c1f`, all **9/9** jobs passed: default/no-default Rust tests, strict Clippy, formatting, docs, no-default dependencies and actual dbt Core E2E with schema/golden regressions. The metadata-only closure commit requires a fresh final-head CI run before merge.

### Matt code-review: two independent axes

- **Standards:** no production semantic parser or adapter heuristic was introduced; independent acceptance oracles use public APIs with fully materialized source relations; NULL/typed sets and accepted interval bounds are checked. The intentionally independent TASK-124 fixture copy is documented as a separate signoff oracle, not a new canonical semantic definition. Conventional commits, no new dependencies, no unsafe, Rustfmt and strict Clippy have been checked. No remaining hard documented-standard violation found.
- **Spec:** all five TASK-124 AC map to the immutable matrix and executable proof tests; tests assert full DuckDB source and terminal bags, selected/rejected physical pairs, per-aggregate group contributor identity, SQL three-valued HAVING, exact output domains, 13 portable parser dialects, compiled dbt and ODCS parity and fail-closed conditional extensions. All source tests, negative empty/NULL/duplicate cases and materialized dbt snapshots passed. No mandatory case has a residual or skipped adapter. No remaining blocking bounded TASK-124 spec gap found.

**Conditional release disposition:** for GROUPING SETS, ROLLUP, CUBE and GROUP BY ALL, parser rejection or canonical typed Residual are the reviewed bounded outcomes. Native vendor/session engine laws and grouping-set identity remain uncertified; TASK-124 owns the pending parent/release disposition, not an assertion of native execution. TASK-73's original six acceptance criteria require **separate independent post-merge parent closure**; TASK-91/release remains separate and unapproved.
