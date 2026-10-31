# TASK-78 independent parent acceptance audit

**Scope:** Independent original TASK-78 acceptance criteria #1..#6, after all four child implementation PRs merged into `main`. This is a parent review gate, not permission to release protocol v3, infer native vendor laws, or skip final PR-head CI.

## Fixed child evidence and immutable parent crosswalk

The [v1 independent parent matrix](task-78-parent-acceptance-freeze.json) indexes **20 mandatory physical/terminal cases** and **9 reviewed conditional boundaries**. Concrete SQL, before/after source bags, typed output domains, TRUE/NOT TRUE proofs and native oracles remain in their versioned child contracts. The parent matrix is a crosswalk, not an alternate implementation of SQL semantics.

| Merged child | Evidence owner | Required / conditional cases | Independent verification |
| --- | --- | --- | --- |
| [TASK-141](task-141-proof-matrix.json), [PR #167](https://github.com/phdah/sql-semantic-protocol/pull/167) | Lexical CTE identity, shadowing, shared inputs, chained producers | 4 / 3 | `tests/task_141_physical.rs`, `tests/task_141_frozen_oracles.rs`, `tests/task_141_adapter_matrix.rs` |
| [TASK-142](task-142-acceptance-freeze.json), [PR #168](https://github.com/phdah/sql-semantic-protocol/pull/168) | Guarded recursive UNION ALL/DISTINCT and termination/absence oracles | 3 / 1 | `tests/task_142_recursive_cte.rs`, `tests/task_142_dialect_adapter.rs` |
| [TASK-143](task-143-acceptance-freeze.json), [PR #169](https://github.com/phdah/sql-semantic-protocol/pull/169) | Filtered and computed producers through shared JOIN/COUNT and target writes | 6 / 3 | `tests/task_143_frozen_matrix.rs`, `tests/task_143_duckdb_oracles.rs`, `tests/task_143_write.rs`, `tests/task_143_adapters.rs` |
| [TASK-144](task-144-acceptance-freeze.json), [PR #170](https://github.com/phdah/sql-semantic-protocol/pull/170) | Reused CTEs, logical views, nested shadowing, ordered materialization and final CTE reads | 7 / 2 | `tests/task_144_frozen_reads.rs`, `tests/task_144_ordered_cte.rs`, `tests/task_144_view.rs`, `tests/task_144_shadow_oracles.rs`, `tests/task_144_adapter_parity.rs` |

All four PRs were merged on `main` before this parent audit. The child audits preserve individual code-head CI and independent review outcomes. The parent integration regression `tests/task_78_parent_closure.rs` guards all 29 case identities and classifications, child freeze versions, mandatory complete terminal bags, typed domains, the six original AC mappings, child-owned conditional cases, dialect identities and separate release gate. It does **not** substitute for independently executing the child-owned Rust and DuckDB tests.

## Independent review of original TASK-78 AC #1..#6

| Original AC | Required evidence | Boundary not promoted to portable Exact |
| --- | --- | --- |
| #1 Named scopes, nesting, LATERAL, views, reuse and ordered relations | TASK-141 REUSE/SHADOW; TASK-144 VIEW/SHADOW/MULTISTEP | TASK-141 materialization and TASK-144 correlated LATERAL/session cases are typed conditional Residuals; a parser or logical view does not prove native materialized execution |
| #2 Recursive CTE proof | TASK-142 ALL/DISTINCT/ABSENCE and TASK-144 FINITE-RECURSION: bounded complete bag and negative/absence truth | TASK-142/144 recursive cycles without a finite termination certificate are Residual, not silently terminated |
| #3 Computed/renamed lineage, fanout, selective producer re-use and grouping | TASK-143 JOIN/COUNT/NULL-DUP and TASK-144 REUSED-FILTER: source-derived exact row identities, typed NULL-inclusive bounds | Opaque expressions cannot borrow an unsupported numeric result or guessed producer rows |
| #4 Source identity and ordered physical read/write | TASK-141 CHAIN; TASK-143 WRITE; TASK-144 MULTISTEP/VIEW: physical input, producer, target and terminal provenance | Forward references with unavailable external source evidence remain unproven; no free intermediate source bag |
| #5 Independent raw/dbt DuckDB oracles | TASK-143 WRITE and TASK-144 MULTISTEP/MULTISTEP-NULL/NULL-DUP: executed complete source/producer/target/terminal bags | Enforced CHECK rejection is a mandatory Impossible case, not an optional boundary; unproved coercion/width laws stay conditional, and raw writes are not fabricated dbt models |
| #6 Dialects, adapters, differential and contract tests | Frozen 14 exposed dialect identities, equivalent direct SQL/compiled dbt/ODCS metadata, child-owned DuckDB tests and active schema/goldens | PostgreSQL alias is an additional identity, not a fourteenth independent native SQL engine. The only independently executed SQL engine is DuckDB; adapter metadata does not imply engine behavior |

### Matt implement review

**Standards axis:** This parent PR changes only task evidence files, the shared coverage registry and a public integration test. Canonical SQL parsing, typed protocol, physical proof and adapter ownership remain unchanged. There is no new dependency, production dialect whitelist, lint waiver, protocol wire change, consumer-side reparsing or speculative source data.

**Specification axis:** The v1 crosswalk enumerates the exact 20 mandatory and 9 conditional child cases, pins a nonempty mandatory reference and reviewed conditional references for each original criterion, checks original case ownership and exact terminal-domain presence and keeps TASK-91 separate. Native DuckDB is supported by existing executable tests rather than by this JSON index. Conditional cases retain their respective TASK-141..144 owner and reviewed dispositions. The parent review does not claim additional LATERAL/session execution proof or arbitrary recursive termination.

**Independent final signoff (2026-10-31):** All six original TASK-78 AC now map to verified mandatory cases and reviewed conditional Residual boundaries. Full nine-job implementation-head CI passed on `fa0b009e7586f760acea6d1397475d59548c81e4`: [run 41242073407](https://github.com/phdah/sql-semantic-protocol/actions/runs/41242073407). Matt's [Standards/Spec review](https://github.com/phdah/sql-semantic-protocol/pull/171#pullrequestreview-5724940824) found zero blocking findings. The review was submitted as COMMENT through the owning GitHub connection, not an independent-account GitHub approval. No native vendor semantics beyond DuckDB were promoted.

## Final signoff gate

The mandatory and conditional evidence, dialect/adapter paths and independent review have been checked against the implementation head. Before merging the final metadata head, rerun: `cargo fmt --check`, strict all-target/all-feature Clippy, `cargo test`, `cargo doc --no-deps`, no-default-feature lint/tests/docs, golden/schema assertions and real dbt Core E2E. The final task-status and coverage-manifest commit must itself pass CI before merge. Parent TASK-78 is now Done; TASK-91 release approval remains separate.
