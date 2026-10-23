# TASK-131 acceptance audit (PR #142)

**Scope:** TASK-75 child 3/4, TASK-131. Original TASK-75 AC #1–#6 remain owned by the parent and are not signed off by this review. The next dependent TASK-132 stays blocked until this child has passed its own final-head gates and merged.

## Freeze and traceability

The task-owned [v5 frozen matrix](task-131-acceptance-freeze.json) retains the original preimplementation IDs `75-C131-P` and `75-C131-N`. The concrete inventory is **12 mandatory** and **3 conditional** cases, **13 configured sqlparser dialects**, and **3 source-evidence adapters**. All **195 parser** cells and **45 adapter** cells are explicitly enumerated, with canonical source and column identities, complete source rows and identities, typed Boolean/integer terminal domains, NULL and inclusive bounds, TRUE/NOT TRUE membership strength, and a precise conditional owner/disposition. The immutable case family originated in v1 before semantic edits; v2 corrected an unsound zero-row filtered scalar example; v3/v4 added independently observed parser and source/terminal oracle evidence without waiving mandatory cells. The explicitly reviewed v5 correction aligns N6/N7 frozen inner source bags with the two duplicate-valued rows used by native DuckDB oracles; no mandatory result or conditional owner changed.

The [canonical coverage manifest](coverage-manifest.json) registers this frozen child. `tests/task_131_scalar_quantified.rs` asserts the inventory and independently verifies all mandatory cases on all 13 dialect parser and canonical-analysis paths. `tests/task_131_adapter_parity.rs` verifies direct SQL, compiled dbt manifest input, and ODCS schema-derived input via the public physical proof API. These are source-evidence adapters; ODCS is not a query execution engine.

## Independent mandatory case review

| Case | Required physical conclusion | Native / canonical oracle |
| --- | --- | --- |
| P1 | Empty nested source, scalar SQL NULL; one complete terminal row | Native DuckDB SELECT and scalar Exact |
| P2 | Singleton 2, exact inclusive [2,2] domain | Native DuckDB SELECT and scalar Exact |
| N1 | Two physical rows (even equal scalar values) cause cardinality error, **not** Impossible | Native DuckDB runtime error and CardinalityViolation proof |
| N2 | Singleton SQL NULL, no numeric bounds | Native DuckDB SELECT and scalar Exact |
| P3 | ANY empty = FALSE, exact Boolean output row | Native DuckDB ANY and canonical SQL-bound Exact |
| P4 | ALL empty = TRUE (vacuous), exact Boolean output row | Native DuckDB ALL and canonical SQL-bound Exact |
| P5 | ANY(NULL,1,1) with operand 1 = TRUE; distinct physical duplicate identities | Native DuckDB ANY and canonical SQL-bound Exact |
| N3 | SOME(1,NULL) with operand 2 = UNKNOWN, **not** FALSE | Native DuckDB SOME and canonical SQL-bound Exact NULL terminal |
| N4 | 5 < ALL(6,NULL,4) = FALSE despite NULL | Native DuckDB ALL and canonical SQL-bound Exact |
| N5 | 5 < ALL(6,NULL) = UNKNOWN | Native DuckDB ALL and canonical SQL-bound Exact NULL terminal |
| P6 | NULL > ANY(4) = UNKNOWN | Native DuckDB ANY and canonical SQL-bound Exact NULL terminal |
| P7 | 5 >= ALL(2,5) = TRUE, exact Boolean output | Native DuckDB ALL and canonical SQL-bound Exact |

All 12 cases read the **complete original physical source bag back from DuckDB**, including row identifiers, nullable values and duplicate memberships, before comparing the independent native SQL outcome against the frozen terminal oracle and the public physical result for every parser dialect. Row-count-only or self-generated oracle checks do not replace native snapshots. Scalar cardinality errors are distinct from impossible source assignments.

## Conditional scope and release disposition

- **N6, nested scalar filtering:** All 13 dialect parsers accept the syntax. Independent DuckDB execution with two equal-valued matching source rows demonstrates a **runtime scalar cardinality error**. The public proof refuses to turn an unmaterialized nested WHERE/control relation into a false Exact result. **Owned TASK-131 Residual** for missing correlated/nested full-bag binding; not a missing mandatory Exact.
- **N7, correlated LATERAL:** All 13 parsers accept the syntax. Independent native DuckDB execution with two matching inner physical rows returns two `o1` output rows, proving per-outer-row fanout and duplicate retention. The SQL-bound quantified helper remains **Residual**, since a single globally evaluated physical inner bag does not certify an arbitrary correlated LATERAL graph. **Owned TASK-131**.
- **N8, vendor APPLY:** All 13 parser dialect configurations accept the syntax, but this proves neither vendor execution support nor portable correlation semantics. Without a trusted native APPLY oracle, **owned TASK-131 Residual**. Other database engines' native semantics and session laws are **not** certified.

The conditional outcomes have a reviewed, task-owned fail-closed disposition with no implicit exclusions. Native execution evidence above exists **only** for DuckDB. Other 12 dialect configurations certify parser and canonical physical laws where shared, **not** native vendor execution. Real native engine-specific APPLY support and cross-vendor session semantics remain separate work, not a prerequisite for this bounded child.

## Two-axis code and acceptance review

**Semantic/code axis:** source-free outer SELECT row-shape must be attested, so even a successful nested scalar is never Exact under outer `LIMIT 0`/`LIMIT 1` without a proven result-row law. Scalar cardinality errors are not Impossible. SQL 3VL preserves UNKNOWN even in SELECT output, never automatically dropping the row. Unknown nested filters, correlations and null-safe quantified operators fail closed. Public protocol normalization owns the operator, quantifier and source identities; the operator-local helper alone is not promoted to whole-SQL exactness.

**Acceptance/oracle axis:** frozen source identities and complete bags, numeric domains and bounds, NULL, duplicate rows, rejected/NOT TRUE strength, error vs Impossible vs Residual, 195 dialect cells, 45 adapter cells, independent DuckDB source/terminal oracles, schema contract, and documentation all have explicit test or review owners. No unchecked conditional is counted as native execution.

**SemVer:** introducing `Predicate::QuantifiedSubquery` and `quantified_subquery` JSON extends a public exhaustive Rust enum and the emitted protocol grammar. Per [AGENTS.md](../AGENTS.md), this is a **breaking public contract change** requiring a major-release classification. The PR's eventual squash commit must carry `feat(protocol)!:` and a `BREAKING CHANGE:` footer. No new version tag or release is produced by this child audit; release planning is independently gated.

## Completion gate

Mandatory/conditional cases and independently evidenced adapters must pass on the **actual final PR head**: `cargo fmt --check`, strict Clippy, full default/no-default Rust test suites, docs, dependency checks, schema/golden contract assertions and `make dbt-e2e`. Keep TASK-131 **In Progress** and PR **draft** if any final-head gate is missing or failed. Once independently green, mark the five child AC as verified, update this audit with the exact CI run and commit, mark TASK-131 Done and make the PR review-ready. TASK-75 and the major-release gate remain independently open.

## v5 corrective verification

- N6/N7 frozen source bags now contain two distinct physical row identities with equal values, matching independently executed DuckDB cardinality/fanout tests; the native tests load the exact frozen bags instead of hand-entered unrelated fixtures.
- Direct SQL, compiled dbt and ODCS conditional boundary regressions now cover all nine N6/N7/N8 conditional adapter cells via public canonical diagnostics or typed scalar residual outcomes. These are classification checks, not native vendor execution.
- A frozen-version consistency guard compares the task-specific freeze against the coverage manifest. The quantified predicate JSON test pins the complete canonical predicate and nested subquery property set.
- The public quantified SELECT now has a complete deterministic JSON golden fixture at `tests/fixtures/task-131-quantified-protocol.json`, asserted against the public emitted protocol. The supplemental SQL-bound WHERE quantified proof retains its independent 13-dialect DuckDB source/terminal and TRUE/NOT TRUE membership regression; this does not enlarge the original 12 mandatory child fixture IDs or count as a native vendor execution claim.

## Final acceptance and two-axis review (2026-10-23)

**Pinned implementation:** `f7c050442c7fd1a3ce5dcf2b1bf6e373994aedc3`. [GitHub Actions CI #39916461940](https://github.com/phdah/sql-semantic-protocol/actions/runs/39916461940) completed **success** on that exact code head, covering formatter, strict Clippy (both configurations), default and no-default Rust tests, docs, dependency checks and dbt Core E2E. The acceptance-status-only follow-up commit must retain a successful CI run before PR review-readiness.

**Original child AC #1:** v5 freeze contains 12 mandatory, 3 conditional cases with both original fixture families, source IDs, typed exact terminal outcomes and domains, 156 required and 39 conditional parser cells, 36 required and 9 conditional adapter cells. Explicitly reviewed v5 N6/N7 correction supplies duplicate physical rows demanded by native oracles. No mandatory cell became Residual.

**Original child AC #2:** The public canonical `QuantifiedSubqueryPredicate` and SQL-bound physical scalar/quantifier planners preserve SQL NULL, vacuous ANY/ALL, NULL-dominance, physical row identity, inclusive integer bounds, terminal Boolean/NULL values and Exact/CardinalityViolation/Impossible/Residual distinctions. Noncertified nested, correlated and vendor-specific execution remains named, fail-closed conditional scope.

**Original child AC #3:** DuckDB checks all 12 mandatory complete physical source bags and terminal outcomes independently; the N6/N7 native conditional oracles load the frozen source populations and prove cardinality failure and duplicate per-outer-row LATERAL fanout. The canonical LATERAL/APPLY test asserts actual typed diagnostics rather than invoking an unrelated quantified projection API.

**Original child AC #4:** All 13 dialect parser paths are enumerated and checked. Direct SQL, compiled dbt and ODCS cover all 36 mandatory and nine conditional adapter cells. Parser acceptance is never presented as execution on any engine other than DuckDB; N8 APPLY lacks a native executable oracle and remains owned TASK-131 conditional residual.

**Original child AC #5:** Complete emitted protocol JSON is pinned by `tests/fixtures/task-131-quantified-protocol.json`; schema, public API tests, all nine CI jobs, and exact-code-head evidence are accounted for above.

**Standards review:** Changes respect canonical semantic ownership, error conservatism, the 13-dialect rule, protocol version provenance, no new dependencies/unsafe/lint ignores, deterministic emission and public-API tests. The intentional breaking exhaustive Rust enum and JSON grammar addition is classified `feat(protocol)!` and needs a `BREAKING CHANGE:` footer on squash. Refactoring beyond this bounded task remains separate work.

**Specification review:** Each child AC maps to directly tested outcomes and every conditional is explicitly classified, not silently excluded. The supplemental source-attested quantified WHERE evaluation is separately tested across 13 parser configurations against independent DuckDB source/terminal TRUE and NOT TRUE membership, without quietly inflating the original 12-case mandatory freeze.

**Disposition:** TASK-131 may be Done after this independent review and passing CI; TASK-75 is **not** signed off, TASK-132 may start only after the child PR merges, and the major-release gate remains open.
