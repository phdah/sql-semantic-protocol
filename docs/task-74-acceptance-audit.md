# TASK-74 independent parent acceptance review

**Status:** Parent closure candidate. All five child tasks are Done and their PRs are merged on `main`; this review does not certify TASK-91, unexecuted vendor engines or the final PR head before CI runs.

## Authoritative contract

The [v1 parent inventory](task-74-acceptance-freeze.json) pins each of the six unchanged original parent acceptance criteria to the existing, versioned child matrices and their canonical oracles. The child inventories own all physical source rows, terminal values, typed bounds and individual parser/adapter classifications. No independent, inconsistent copy of their expected outcomes is maintained.

| Merged child | PR | Owned semantics | Full-bag and negative evidence |
| --- | --- | --- | --- |
| TASK-125 | #135 | ROW_NUMBER, RANK, DENSE_RANK; peers, direction, partition and NULLs | `tests/task_125_rank_bags.rs` |
| TASK-126 | #136 | LAG, LEAD, FIRST/LAST_VALUE and bounded ROWS; empty forward frame | `tests/task_126_window_bags.rs` |
| TASK-189 | #137 | COUNT(*), COUNT(expr), SUM/AVG/MIN/MAX; partition, running, moving, forward; typed fractional DECIMAL | `tests/task_189_window_aggregates.rs`, `tests/task_189_decimal_windows.rs` |
| TASK-127 | #138 | Multi-condition QUALIFY, JOIN and grouped producer composition, complete TRUE/NOT TRUE identities | `tests/task_127_qualify_bags.rs`, `tests/task_127_upstream_native_oracles.rs` |
| TASK-128 | #139 | NTILE, GROUPS/RANGE peer frames, collation/session boundaries and materialized ranked_orders | `tests/task_128_advanced_windows.rs`, `tests/task_128_dbt_core_e2e.rs` |

The inventory pins **34 mandatory Exact** and **22 reviewed conditional** individual cases across all five freezes, plus each original positive/negative family ID. All five freezes enumerate the **13 parser dialect names** and **three adapter identities**; the dbt alias used by TASK-128 is normalized at the parent boundary. Per-child adapters and per-case parser classifications remain checked by the existing child executable suites.

## Original acceptance criteria

| AC | Independently reviewed evidence | Explicit limits |
| --- | --- | --- |
| #1 ranking, navigation and framed aggregates | TASK-125 peer ranking; TASK-126 navigation/ROWS; TASK-189 exact integer/finite DECIMAL aggregates; TASK-128 bounded advanced NTILE and peer frames | DISTINCT/FILTER over windows without certified physical/engine laws remain typed Residual, never exact |
| #2 PARTITION/ORDER/frame and peer/NULL laws | TASK-125 tied order, ASC/DESC and explicit NULLS; TASK-126 and TASK-189 explicit ROWS; TASK-128 peer-aware RANGE/GROUPS | Implicit default NULL placement, arbitrary numeric RANGE offsets and collation/session comparison remain non-Exact |
| #3 QUALIFY and composed producers | TASK-127 conjunctions of multiple windows, full joined and grouped upstream bags and their positive/rejected identities; TASK-189 aggregate QUALIFY and TASK-128 NTILE/RANGE QUALIFY | Untested producer shapes and underordered ROW_NUMBER remain Residual; existing `tests/window_witness.rs` covers projected-window alias predicates separately |
| #4 nondeterminism | TASK-125/126/189/127 prove underordered peer limits, TASK-128 refuses unsupported NTILE tie/implicit session laws | No arbitrary tied ROW_NUMBER winner or unexecuted vendor claim |
| #5 independent DuckDB native execution and ranked_orders | The five child native full-source/terminal suites; TASK-127 P4 and P5 are independently re-executed by `tests/task_74_parent_closure.rs`; TASK-128 dbt Core executes `ranked_orders.sql` | DuckDB native result is not evidence for 13 distinct SQL engines |
| #6 typed domains, adapters, parser and negative regression | The parent guard rejects missing/misclassified child cases, invalid AC mappings, absent typed terminal oracles and truncated dialect inventories; child suites independently test public protocol, adapters and golden/schema regressions | SQLite numeric oracles are native only where specifically executed; remaining vendor/session laws are owner-reviewed non-Exact |

## Parent-owned execution regression

`tests/task_74_parent_closure.rs` executes two cross-operator cases from the previously merged TASK-127 frozen matrix without replacing their canonical fixtures:

- `74-C127-P4`: two independent physical JOIN source bags, qualifying full joined row identities, complete SQL terminal bag and typed inclusive bounds
- `74-C127-P5`: six physical source rows, GROUP BY/COUNT(*) producer, tie-broken QUALIFY, complete terminal bag, precise NULL/domain evidence

For both, the parent regression creates and reads back every physical DuckDB source row before executing the actual SQL, compares complete terminal *multisets* and cross-checks the frozen inclusive typed domains. That is an additional independent native execution boundary rather than metadata-only signoff.

The invariant test also pins every mandatory and conditional case ID from TASK-125/126/189/127/128, including the five finite-DECIMAL Exact cases and two numeric Residual counterexamples in TASK-189's secondary matrix. It rejects missing child fixture IDs, missing typed domains, incomplete 13-dialect coverage and an unchecked original AC mapping.

## Reviewed conditional disposition

Conditional underordered peers, implicit NULL ordering, COLLATE/session rules, unbounded or offset RANGE, unsupported FILTER/DISTINCT, overflow/decimal promotion and vendor-specific output types remain owned by their respective child task. Residual classification is not used for any required bounded Exact case. Parser support across 13 dialects does not certify runtime engine support in 13 engines; DuckDB native execution and the expressly pinned SQLite numeric checks are the only native evidence counted here.

## Two-axis review and merge gate

**Standards:** Parent evidence lives in a single JSON inventory, an independent executable integration test, the task tracker, code map and coverage manifest. The production parser, canonical protocol and emission contract remain unchanged. No extra dependency, guessed semantics or additional public wire shape is introduced.

**Spec:** All six original TASK-74 criteria are mapped to existing merged child evidence; the critical upstream JOIN/GROUP-to-QUALIFY paths receive an additional parent-owned DuckDB full-bag re-execution. No child is reopened. TASK-91 remains a separate release authorization gate.

Run `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, schema/golden checks and real `make dbt-e2e` on the final implementation and acceptance-metadata PR head. **Do not mark TASK-74 Done or merge this PR until that verification succeeds.**
