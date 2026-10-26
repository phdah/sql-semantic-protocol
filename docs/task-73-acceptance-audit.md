# TASK-73 independent parent acceptance audit

**State:** Original AC #2's cross-operator evidence is now mapped to already merged TASK-115 exact three-source/projected-group cases and an executable parent oracle; signoff remains pending final PR-head CI. TASK-73 remains In Progress until the final independent verification and TASK-91 remains a separate release gate.

## Source of truth

The [versioned parent freeze](task-73-acceptance-freeze.json) owns the six original AC mappings and pins eight child fixture families, 12 exact source/terminal cases and eight conditional boundaries. It references the merged, independently frozen TASK-121..124 cases without copying their expected physical bags. The public guard is `tests/task_73_parent_closure.rs`.

All four child PRs are merged on `main`: #131 (basic aggregates), #132 (DISTINCT/FILTER/HAVING), #133 (filtered joins), #134 (independent full-bag/dialect/adapter audit). The independent oracle is `tests/task_124_parent_aggregate_oracles.rs`, adapter parity is `tests/task_124_aggregate_adapters.rs`, and the materialized `aggregate_summary.sql` oracle is `tests/protocol_conformance_dbt_e2e.rs`.

## Original acceptance review

| AC | Mandatory evidence | Reviewed, non-promoted boundary |
| --- | --- | --- |
| #1 COUNT/SUM/MIN/MAX/AVG, aliases, DISTINCT/FILTER/HAVING | Global/grouped nullable duplicates; DISTINCT, FILTER and Boolean HAVING exact frozen bags | FILTER's dialect-specific parser boundary does not imply a runtime guarantee |
| #2 WHERE/JOIN/projection and group provenance | Complete direct two-relation join, duplicate amplification, rejected group, empty global join and per-source IDs | **Parent evidence added, pending CI:** TASK-115's three-source two-join planner and projected stage GROUP/HAVING positive/rejected exact cases. Parent-owned oracle executes both cases independently; child 13-dialect and 3-adapter proofs remain authoritative. |
| #3 Empty groups and grouping extensions | Empty global yields one group; empty GROUP BY yields none; an independent DuckDB oracle distinguishes real NULL keys from subtotal NULLs | GROUPING SETS, ROLLUP, CUBE, GROUP BY ALL and GROUPING() stay parser-bound or typed Residual/explicit unsupported in uncertified engines |
| #4 Positive/rejected typed domains and overflow | Exact typed inclusive bounds, NULL sets and TRUE/FALSE/UNKNOWN HAVING, closed physical and terminal bags | Nonintegral AVG, overflow and unproven numeric/collation semantics remain explicitly non-Exact |
| #5 DuckDB and dbt independent execution | 12 complete DuckDB physical/terminal oracles and real materialized `aggregate_summary.sql` expected `north:1:42` | No fabricated database-native outcomes outside DuckDB |
| #6 Dialects, adapters, public contract and regressions | 156 mandatory 13-dialect parser classifications; 36 SQL/dbt/ODCS adapter cells; schema and golden CI | 65 conditional parser and 15 adapter boundary cells remain non-Exact |

The required numeric result and complete input/output bag expectations are owned by the child freeze; this parent inventory pins fixture IDs, original AC mapping, case counts, proof class and owners. The regression guard rejects removal of a mandatory fixture, omission of a dialect/adapter, unreviewed Exact promotion and coverage-manifest drift. The parent-owned GROUPING() boundary separately checks a complete DuckDB bag and its non-Exact classification. The existing executable child tests must also pass; parent metadata alone does not certify semantics.

## Conditional disposition

The four grouping extensions are owned by TASK-124. The GROUPING() bit-function boundary is owned by TASK-73; its independent DuckDB oracle distinguishes an ordinary NULL group from the synthetic NULL in a grand-total row without promoting unsupported portable or vendor semantics. Unknown subtotal/group identity, inferred grouping keys and vendor/session behavior are non-Exact pending a separately attested native execution law. Three numeric boundaries (nonintegral AVG scaling, SUM overflow and noninteger/decimal/collated MIN/MAX) remain owned by TASK-121 and fail closed. Their presence does not turn an unsupported mandatory portable case into a permissible residual. SQL parsing in 13 dialects is not evidence of 13 real native engines.

## Review and merge gate

**Standards axis:** This change is confined to a parent acceptance freeze, public integration guard, audit, coverage registration and Backlog.md status. No production semantic rule or protocol shape changes, no new crate dependency, and no duplicate bag oracle.

**Spec axis:** Each original AC has a pinned case mapping, but the finite portable floor does not discharge every literal requirement of AC #2. The remaining upstream projection-layer and multi-join paths need an exact canonical source/terminal oracle and corresponding dialect/adapter evidence before AC #2 or the parent can be signed off. The eight conditional boundaries remain non-Exact and owned.

Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo doc --no-deps`, no-default-features tests, schema/golden checks and real dbt E2E on the final PR head. All nine implementation-head jobs passed in [CI 40404594315](https://github.com/phdah/sql-semantic-protocol/actions/runs/40404594315) at `64599c7b10de1361f0a51011d489db1d19023365`. The 12 frozen core cases remain evidenced, but independent Spec review identified the unverified upstream projection/multi-join requirement in original AC #2. Keep AC #2 unchecked, parent In Progress and parent_signed_off=false until it is implemented and independently tested. Final PR-head CI remains required. This PR does not approve TASK-91.

## Outstanding parent requirement

The original two-source aggregate planner remains intentionally restricted. TASK-115 already provides the distinct three-source, two-join canonical physical planner, including a projected CTAS producer and downstream GROUP/HAVING. Parent acceptance pins its two original immutable grouped P/N cases under upstream_composition: 26 parser cells and six adapter cells (additional to TASK-124's 156 and 36) plus the independent new parent executable oracle. Child source/terminal expectations retain canonical ownership. Signoff awaits green parent-head CI.
