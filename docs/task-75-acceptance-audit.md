# TASK-75 independent original acceptance audit

**Decision:** Original AC #1..#6 are independently reconciled for the bounded portable SQL proof contract. All four child tasks are Done and their PRs are merged. TASK-91 remains the separate protocol 3.0.0 release gate; parser compatibility is never native vendor execution certification. The final parent-PR CI must pass before merge.

## Fixed evidence and verification

The [v1 parent acceptance inventory](task-75-acceptance-freeze.json) pins all **40 required case outcomes** from the four independently frozen child contracts: **39 Exact physical/terminal outcomes and one explicitly expected scalar CardinalityViolation**. It separately preserves **10 reviewed conditional/boundary cases**. The original child bags, domains and expected SQL results are not copied into the parent freeze.

| Child | Merged PR | Mandatory outcomes and independent execution |
| --- | --- | --- |
| [TASK-129](task-129-acceptance-audit.md) | [#140](https://github.com/phdah/sql-semantic-protocol/pull/140) | Six correlated/uncorrelated EXISTS/NOT EXISTS complete-bag oracles; duplicates, NULL keys, empty/nonempty candidate sets and closed-world NOT TRUE; `tests/task_129_exists_bags.rs` |
| [TASK-130](task-130-acceptance-audit.md) | [#141](https://github.com/phdah/sql-semantic-protocol/pull/141) | Fifteen scalar/tuple IN/NOT IN and compound-key cases, SQL three-valued truth, candidate identity and duplicate/NULL cases; `tests/task_130_in_bags.rs` |
| [TASK-131](task-131-acceptance-audit.md) | [#142](https://github.com/phdah/sql-semantic-protocol/pull/142) | Twelve mandatory scalar/ANY/SOME/ALL cases, including zero/one result, NULL, duplicates, empty ANY/ALL and **scalar cardinality error distinct from source Impossibility**; `tests/task_131_scalar_quantified.rs` |
| [TASK-132](task-132-acceptance-audit.md) | [#143](https://github.com/phdah/sql-semantic-protocol/pull/143) | Seven direct composite-key, joined/grouped, rejected and multi-producer dbt cases; `tests/task_132_composite_correlations.rs`, `tests/task_132_adapter_parity.rs` |

The 40 required case outcomes account for **520 explicit parser/canonical dialect cells** and **120 source-adapter parity cells**. Child oracles validate complete source and terminal DuckDB relations and row identities, typed NULL and inclusive numeric bounds, and SQL TRUE/NOT TRUE or error status as appropriate. The native execution corpus is DuckDB; the 13 dialect names are parser/canonical paths, not 13 native warehouse certifications.

The independent [parent closure regression](../tests/task_75_parent_closure.rs) reads the four immutable child inventories through their canonical files, enumerates every required and conditional case, rejects missing physical bags, terminal domains or runtime-cardinality error oracles, and requires complete 13-dialect and three-adapter matrices. It checks the six original AC crosswalks and their coverage-manifest registration, and prevents a conditional `Residual` from silently becoming a mandatory `Exact` conclusion. The semantic and DuckDB assertions continue to run in their child-owned public API and execution tests.

## Original parent AC review

| Original AC | Required evidence | Conditional disposition |
| --- | --- | --- |
| #1 Subquery forms and correlation | TASK-129 EXISTS/NOT EXISTS, TASK-130 scalar and compound-key membership, TASK-131 scalar/quantified comparison and TASK-132 nested GROUP/JOIN, composite correlations | TASK-131 N6/N7/N8 nested scalar, LATERAL and APPLY shapes remain explicitly conditional where physical binding or native vendor law is unavailable |
| #2 Complete multiplicity, NULL and anti-membership | Exact duplicate/empty/NULL candidate bags, full TRUE versus NOT TRUE outer identities, correlated tuple negation, and scalar 0/1/multiple-row contrast; multiple scalar rows produce a runtime cardinality error, not `Impossible` | DuckDB tuple-NULL `IN` does not substitute for portable SQL row equality; the independent expanded fieldwise DuckDB reference is authoritative |
| #3 LATERAL/APPLY and source scopes | TASK-132 complete joined and grouped producer/source identity proves the portable correlated core without invented physical intermediates | Parser-accepted correlated LATERAL/APPLY and vendor/runtime session laws remain reviewed typed `Residual` until executed/attested per engine |
| #4 Exact, rejected and typed proof | All selected/rejected identities and candidate bags are exhaustive; bounded domains use typed values and inclusive limits. Missing metadata, unproved comparison and nested graph laws fail closed | Noninteger tuple equality, collation, unproved nested producer forms and vendor session properties are not asserted as portable `Exact` |
| #5 DuckDB and dbt oracle | Complete independently executed DuckDB cases in TASK-129..132; compiled `subquery_orders.sql` traverses materialized raw, staged, joined and terminal producer bags and compares terminal `[(amount=42, region=north)]` | ODCS supplies equivalent schema evidence; it is not a runtime engine |
| #6 Dialect/adapter and protocol contract | 13 parser identities and direct SQL/dbt/ODCS canonical parity per mandatory case, strict Rust tests, typed outcome contract, no-default-features, dbt Core E2E and active JSON schema/golden gates | None of these claims certifies an unexecuted native vendor SQL law |

The parent does not silently waive mandatory Exact cases: every named portable outcome has a concrete source/terminal fixture, matching public proof test and native DuckDB oracle. The single required scalar error is a verified runtime outcome, not a fabricated empty bag or a wrongly classified impossible source. Conditional cases retain their original owning children and reviewed release dispositions.

## Matt skill implementation review

**Standards axis:** The PR adds a versioned parent crosswalk, public integration guard, coverage registry, audit and Backlog.md closure. The finite physical witnesses remain owned by canonical protocol/physical modules and their task-specific tests. It adds no production SQL parser logic, dependency, duplicated oracle bag, `unsafe`, public schema change or arbitrary SQL dialect whitelist. Child parser spellings are normalized only inside the test assertion (`postgres` to `postgresql`); production runtime dialect behavior is unchanged.

**Spec axis:** The independently merged TASK-129..132 work covers all original TASK-75 AC, with exact source/terminal evidence and explicit reviewed native limits. The parent regression fails if a mandatory cell, dialect/adapter classification, source or terminal proof, conditional owner, or original AC mapping disappears. The task-specific matrix is not an excuse to promote unverified vendor/LATERAL semantics. TASK-75 Done signifies the portable original acceptance, not release approval or generalized opaque nested SQL support.

## CI and merge gate

Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo doc --no-deps`, no-default-features tests/lint/docs, schema/golden checks, and actual dbt Core E2E on the **final PR head**. All child semantic heads had successful nine-job CI before merge (see each child audit). The parent metadata/test head is subject to its own final CI. TASK-76 and TASK-77 still require independent closure before TASK-141 can start; TASK-91 remains a separate release gate.
