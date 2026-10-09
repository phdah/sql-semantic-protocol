# Decisions requiring maintainer approval

**Status: proposals only. Nothing below has been approved.** These decisions establish the finite support envelope and generator acceptance rules, not a claim that SQL Semantic Protocol 3.0.0 or sql-tdg m-3 is ready to release.

Evidence: [coverage matrix](coverage.md), [machine-readable inventory](coverage-manifest.json), protocol [TASK-66](../.backlog/tasks/task-66%20-%20inventory-sql-dialect-feature-contract.md) and [TASK-91](../.backlog/tasks/task-91%20-%20gate-single-protocol-3-release-on-tdg-signoff.md).

## Decision 1: What is a rejected source row in whole-project mode?

**Recommendation: per-terminal-outcome classification**, rather than requiring a source row to be absent from *every* terminal output.

- Every generated physical row receives a vector of exact, independently verified membership/absence claims for each selected terminal output.
- For terminal `T`, a negative row is a physical input row proven **not to contribute to any output row of `T`**. It may legitimately contribute to other terminal outputs. The vector must identify those terminals.
- Matching source rows must still satisfy all intended positive terminal obligations. The existing CLI default `--rejected 10` must work in whole-project mode, with consistent per-source requested counts and no fabricated claims when targets share data.
- Duplicate values, null semantics, joins, fan-out, CTE producers, and post-DML state must not turn a negative case into a positive by unaccounted-for alternative input paths. If exact absence cannot be proven for a requested terminal, **fail naming that terminal**, never silently downgrade or return fewer rows.
- Execute all selected terminal SQL against the same generated dataset and compare the complete per-terminal membership vector, not only row counts.

**Alternative:** globally rejected rows, excluded by all terminals; simpler but misses `passes A/fails B` cases. Requires changing sql-tdg TASK-35/36 acceptance.

Maintainer approval: **pending**. Owner: sql-tdg TASK-35; protocol TASK-86, TASK-68, TASK-91.

## Decision 2: Where should INSERT/UPDATE/DELETE/MERGE acceptance live?

**Recommendation: a dedicated, required DuckDB DML state-transition E2E job** outside the pure dbt model DAG. Its artifact/log must be included in the overall m-3 acceptance gate alongside dbt `make all`.

- Generate physical sources and initial target state from the protocol's canonical obligations, execute complete ordered SQL mutation programs, then verify precise post-state contents and positive/negative rows.
- Cover keys, conflicts, untouched rows, matched/unmatched MERGE branches, NULLs, non-idempotent INSERT, and idempotence **only where provable**.
- Include dialect-specific syntax under separate parse/analysis tests. The DuckDB oracle cannot certify vendor-specific execution behavior. Other engines require their own executed oracle or approved exclusion.
- The committed dbt fixture's `make all` continues to own SELECT/model graph, group/window/set and rejection tests. Its CI aggregate is not green unless the separate DML job also passed.

**Alternative:** build a dbt incremental-model or snapshot fixture expressing each DML variant. This cannot universally represent DELETE/MERGE/DDL syntax and would need explicit external-case coverage anyway.

Maintainer approval: **pending**. Owner: sql-tdg TASK-31 and TASK-36; protocol TASK-80..84, TASK-89/91.

## Decision 3: Bounded support rather than `any SQL`

**Recommendation: explicitly exclude only opaque, inherently unprovable or unavailable semantics, with fail-closed proofs**.

- Arbitrary UDFs, external side-effecting functions and runtime environment calls without a declared deterministic algebra.
- Unseeded stochastic sampling, random ordering, nondeterministic/tie-ambiguous result selection.
- Potentially nonterminating/unbounded recursive statements without a finite, provable termination/boundary.
- Vendor-specific SQL operators and implicit collation/timezone/coercion laws without declared executable semantics.

**Important:** approval does *not* exclude entire classes such as `WITH RECURSIVE`, vendor statements or expressions that *can* be modeled exactly under explicit bounded assumptions. Their safe subsets remain in-scope under TASK-70..88. Parseable but unsupported forms must emit a typed residual/unsupported diagnostic, and sql-tdg must refuse exact generation. Unparseable forms fail with explicit parser errors; neither is classified as passing coverage. All exclusions require a documented negative test, explicit scope record and review.

**Alternative:** treat every extension as release blocking. That would make the scope unbounded and the release effectively unverifiable.

Maintainer approval: **pending**. Owner: protocol TASK-66, TASK-78, TASK-88, TASK-91.

## Decision 4: Executable dialect certification

**Recommendation: distinguish 13-dialect parser/semantic coverage from SQL-engine certification.** Use DuckDB for directly executable and equivalent SQL shapes; do not claim DuckDB establishes Snowflake, PostgreSQL, MySQL, SQL Server, BigQuery or other vendor runtime laws.

- Common SQL syntax must be parsed and analyzed across all exposed dialect names where sqlparser accepts it, with corresponding AST and normalized-result tests.
- Dialect-specific forms need explicit parser-boundary and semantic evidence, including conditional NULL ordering, collation, timestamp zones, arithmetic overflow, and DML/DDL semantics.
- Exactness dependent on a vendor runtime setting remains residual without declared and independently verifiable assumptions.
- For unavailable vendor engines, record **engine oracle unavailable / not certified** and seek explicit approval to ship only dialect-independent safe semantics, with the corresponding unsupported settings failing closed.
- Adding an executed vendor engine later must include its version/session configuration and SQL result snapshots, not just a parser test.

**Alternative:** require native execution of all supported dialects and engine versions before 3.0.0. This is stronger but introduces vendor infrastructure and potentially licensing dependencies.

Maintainer approval: **pending**. Owner: protocol TASK-88, TASK-89, TASK-91; sql-tdg TASK-33/36.

## Final release sign-off, a separate future gate

**Do not sign off on releasing protocol 3.0.0 yet.** Final approval is valid only when protocol TASK-66..90 are Done, approved exclusions are audited, CI is green, sql-tdg has pinned the candidate Git SHA, generated a complete positive/negative dataset, passed dbt `make all`, verified DML results and full cross-feature output oracles, and TASK-91's acceptance is complete. Release Please PR #79 remains unmerged until that point.

To approve the scope *now*, the maintainer may explicitly accept decisions **1–4** (individually or together). Approval establishes what engineering/tests must implement; it **does not waive** the acceptance tests or prove readiness to release.
