# TASK-74 independent parent acceptance audit

**State:** Original parent AC #1..#6 are independently signed off for the frozen portable scope. The five child tasks are Done and merged. Final PR-head CI remains the merge gate; TASK-91 independently controls protocol v3 release authorization.

## Scope and frozen evidence

The [v3 parent acceptance inventory](task-74-acceptance-freeze.json) is the machine-readable owner of original AC #1..#6 mapping, mandatory/conditional classification and independent parent review. It preserves the child freezes as canonical owners of their exact source and terminal bags. Across the five merged children it references 34 Exact cases and 22 reviewed conditional cases. The separate [filtered WHERE/QUALIFY matrix](task-74-filtered-qualify-freeze.json) adds four Exact and one reviewed Residual, totaling **38 mandatory Exact** and **23 conditional** cases.

| Child | Merged PR | Audited contract and independent native oracles |
| --- | --- | --- |
| TASK-125 | #135 | ROW_NUMBER, RANK, DENSE_RANK, peer order and NULLs; `tests/task_125_rank_bags.rs` |
| TASK-126 | #136 | LAG/LEAD, FIRST/LAST_VALUE and bounded ROWS frames; `tests/task_126_window_bags.rs` |
| TASK-189 | #137 | COUNT/SUM/AVG/MIN/MAX aggregate windows and finite DECIMAL/NULL domains; `tests/task_189_window_aggregates.rs`, `tests/task_189_decimal_windows.rs` |
| TASK-127 | #138 | Joined/grouped producers, conjunctive ranking QUALIFY, exact selected/rejected identities; `tests/task_127_qualify_bags.rs`, `tests/task_127_upstream_native_oracles.rs` |
| TASK-128 | #139 | NTILE, GROUPS/RANGE, explicit NULL order, ranked_orders.sql native dbt model; `tests/task_128_advanced_windows.rs`, `tests/task_128_dbt_core_e2e.rs` |

## Original criteria independent crosswalk

| AC | Mandatory evidence | Conditional boundary |
| --- | --- | --- |
| #1 Window functions | Exact rank/navigation/analytic COUNT/SUM/AVG/MIN/MAX and bounded advanced-frame cases in children 125/126/189/128 | DISTINCT/FILTER and unattested function/engine types remain non-Exact |
| #2 Order and frames | Deterministic explicit partition, ASC/DESC, tie laws, ROWS, RANGE and GROUPS evidence in all five children | Implicit NULL order, COLLATE, numeric offset and session laws remain reviewed Residual |
| #3 QUALIFY composition | TASK-127 joined and grouped complete upstream rows; four Exact parent WHERE-filtered cases; TASK-189 aggregate QUALIFY; TASK-128 NTILE/RANGE QUALIFY; independent outer projected ROW_NUMBER predicate | Underordered physical peer identity and unsupported producer expressions remain Residual |
| #4 Nondeterminism | Stable rank peer laws and explicit rejection of underordered ROW_NUMBER/NTILE outcomes | Non-unique ORDER BY and vendor-dependent comparator/session claims remain non-Exact |
| #5 Engine oracles | Native DuckDB complete source/terminal bags for five children, parent JOIN/GROUP and WHERE cases, and real ranked_orders dbt Core E2E | 13 parser dialects do not certify 13 native SQL engines |
| #6 Cross-dialect, adapters and output domains | 13 parser identities, direct SQL/compiled dbt/ODCS parity, typed inclusive output domains, NULL/Impossible/Residual regressions, schema/golden CI | Native vendor runtime behavior is not inferred from parser support |

## Parent-owned independent execution

`tests/task_74_parent_closure.rs` re-executes frozen TASK-127 P4 and P5 joined/grouped QUALIFY complete DuckDB source and terminal multisets with precise typed output bounds. It additionally validates an outer projected ROW_NUMBER filter against the public SQL analyzer and full DuckDB TRUE and NOT TRUE bags. The executable parent guard verifies each child's frozen variant identity, source/terminal and domain evidence, all required parser/adapter names, original AC mapping, and the mandatory/conditional union.

The [frozen filtered matrix](task-74-filtered-qualify-freeze.json) contains four Exact SQL cases: nullable signed-integer comparison with duplicate peer values (P1), SQL `IS NULL OR` three-valued selection (P2), conjunctive `AND` (P3), and a zero-row WHERE-TRUE partition (N1). Its computed-expression case (C1) is explicitly Residual. `tests/task_74_filtered_qualify.rs` independently asserts full physical and terminal DuckDB bags, WHERE TRUE versus FALSE/UNKNOWN physical identities, QUALIFY TRUE versus NOT TRUE identities, 13 canonical parser dialect paths and exact finite typed inclusive output domains. `tests/task_74_filtered_adapters.rs` checks equivalent direct SQL, compiled dbt manifest and ODCS source schema evidence against the same public physical API.

The canonical evaluator first compiles every WHERE branch against authoritative signed-integer column types and identities, then evaluates complete SQL three-valued TRUE/FALSE/UNKNOWN physical populations. Windows partition only WHERE-TRUE rows. Original physical rows stay in `source_row_ids`; WHERE-not-TRUE rows are retained in `rejected_upstream_row_ids`, QUALIFY-not-TRUE in `rejected_row_ids`, and survivors in `terminal_row_ids`. Unsupported computations never silently become false or Exact. The parser separately certifies a WHERE+QUALIFY direct-table shape without loosening JOIN, DISTINCT, grouping, TOP or LIMIT safety rules.

## Review

**Standards:** Narrow parser certification and one typed physical filter module, no unsafe, no new dependency and no changed public JSON protocol schema. Deterministic identities and value domains come from the same complete source bag. Unsupported and vendor/session cases fail closed.

**Spec:** All six original AC have executable mandatory evidence. The previously missing filtered upstream input is implemented and native-oracle tested. The reviewed conditional cases are not used to excuse any missing Exact outcome. TASK-91 remains independently release-blocked.

**Verified implementation head:** [CI 40484240246](https://github.com/phdah/sql-semantic-protocol/actions/runs/40484240246) passed 9/9 jobs: cargo fmt, strict Clippy both feature configurations, full tests both configurations, Rust docs both configurations, no-default dependencies and executed dbt Core E2E. Full tests include schema/golden and native DuckDB regressions.

**Merge gate:** final acceptance-metadata PR head must pass the same CI before merge. TASK-74 is Done for the portable scope; it does not authorize protocol 3.0.0 or native vendor runtime certification.
