# TASK-123 final bounded acceptance audit

Date: 2026-10-19
Owner: TASK-123. Parent: TASK-73 (remains To Do).
PR: https://github.com/phdah/sql-semantic-protocol/pull/133
Frozen inventory: [task-123-acceptance-freeze.json](task-123-acceptance-freeze.json), version 1.

## Independently reviewed mandatory physical cells

| Case | Complete source bag and pair evidence | SQL group and terminal oracle |
| --- | --- | --- |
| `73-C123-P-join` | Four key=1 joined pairs and one key=2 pair survive WHERE; all seven other physical Cartesian pairs are NOT TRUE (including NULL equality). | Group 1 passes HAVING with (k=1, row_count=4, count_x=2); group 2 fails. The terminal is exactly one row. |
| `73-C123-N-rejected` | Same five selected and seven NOT TRUE pairs, preserving duplicate identities and NULL rejection. | Both groups fail HAVING COUNT(*) > 4; terminal bag and output domains are empty. |
| `73-C123-P-empty-global` | Empty left bag, nonempty right bag, no selected or rejected Cartesian pairs. | Global COUNT(*) emits exactly one row, row_count=0, with inclusive finite output domain {0}. |

The complete frozen source bags, stable pair IDs, group identities, contributor IDs, full terminal bags, NULL membership, inclusive extrema and typed finite domains were reviewed against the implementation and integration assertions. `tests/task_123_join_aggregates.rs` independently reads back both entire DuckDB input bags and executes the terminal SQL for all mandatory cases. It checks all selected/rejected source pairs, the full terminal multiset, contributor identities, HAVING truth, NULL membership and included extrema. Empty-bag unsupported integer/Boolean comparisons also return Residual instead of an unsound Exact proof.

## Child acceptance mapping

1. **Freeze:** Three mandatory and four conditional cases with fixed IDs, SQL, physical populations, canonical identities, dialect/adapter cells, proof strength and named release boundaries are registered via `docs/coverage-manifest.json`. The frozen v1 fixture file is unchanged after semantic implementation.
2. **Canonical semantics:** `src/physical_realization/aggregate_joins.rs` evaluates exactly two attested physical sources under an INNER JOIN, WHERE, an ordinary integer group key or a global aggregate, and HAVING. COUNT, typed SUM/MIN/MAX/AVG, distinct COUNT and aggregate FILTER are supported where arithmetic is provable. Every Cartesian pair is TRUE-selected or NOT TRUE-rejected; groups, contributors, multiplicities, terminal bags and exact finite ValueDomain sets share the same evidence. Contradictory input schema is Impossible, missing or unproved law is Residual.
3. **Independent oracle:** All three mandatory terminal/source bags were checked with DuckDB, including NULL join rejection, duplicated contributors, a HAVING nonmembership counterexample and empty-global singleton behavior.
4. **Dialect and adapters:** The complete 3 x 13 mandatory parser dialect matrix is required to return Exact; all 39 cells passed. The four conditional grouping families across 13 dialects (52 cells) must resolve to a named parser boundary or typed Residual, never false Exact. For equivalent typed source evidence, the 3 x 3 mandatory SQL/dbt-compiled/ODCS adapter proofs return identical physical and terminal results; the 4 x 3 conditional adapter checks similarly prohibit Exact. Parser failure is an explicit grammar boundary, not evidence for engine execution. GROUPING SETS, ROLLUP, CUBE and GROUP BY ALL retain TASK-123 ownership and a reviewed conditional nonrelease disposition until grouped subtotal identities, GROUPING bits and per-dialect engine/session behavior are certified.
5. **Verified CI:** 9/9 GitHub Actions jobs passed on semantic head `84e9a0345591a36fbf51bd484731f44a33824f83`: [run 39305376784](https://github.com/phdah/sql-semantic-protocol/actions/runs/39305376784). Coverage includes rustfmt, strict Clippy (default and no-default-features), full Rust tests (default and no-default-features), docs, dependency checks and dbt E2E. Final metadata-only signoff-head CI remains a premerge check.

## Deliberate scope boundary

This child certifies direct two-source INNER JOIN-to-group composition, not arbitrary joins, projection chains or nested grouping-set subtotal laws. Decimal/overflow and native vendor/session execution are not guessed. Those unsupported laws remain explicitly Residual, with TASK-124 and the original TASK-73 parent acceptance criteria still open for independent closure. This is child signoff only, not a v3 protocol release or parent signoff.
