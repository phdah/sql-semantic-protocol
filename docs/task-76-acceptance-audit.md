# TASK-76 independent parent acceptance audit

**Scope:** Original TASK-76 AC #1..#6, after all four child tasks were marked Done on `main`. This review does not authorize the TASK-91 release or certify unexecuted vendor runtime laws.

## Frozen parent matrix

[Version 1](task-76-acceptance-freeze.json) independently reconciles **26 mandatory Exact** and **16 reviewed conditional** SQL/data cases from the versioned TASK-133, TASK-134, TASK-135, and TASK-136 acceptance freezes. It preserves the child-owned SQL, typed source/terminal domains, stable row identities, selected/NOT TRUE rows and complete independent DuckDB oracles rather than copying or weakening their expected results.

| Owner | Exact | Conditional | Core evidence |
| --- | ---: | ---: | --- |
| TASK-133 | 9 | 4 | Total-order LIMIT/OFFSET, FETCH, TOP, NULL ordering, empty/Impossible, [independent audit](task-133-acceptance-audit.md) |
| TASK-134 | 6 | 3 | WITH TIES, DISTINCT/DISTINCT ON, NULL and tied winner boundaries, [independent audit](task-134-acceptance-audit.md) |
| TASK-135 | 6 | 3 | Nested CTE and set-branch pagination, complete producer/source identity, empty and rejected outputs, [independent audit](task-135-acceptance-audit.md) |
| TASK-136 | 5 | 6 | Deterministic dialect equivalents and fail-closed seeded/stochastic sampling, [independent audit](task-136-acceptance-audit.md) |

Every mandatory variant is linked to its authoritative child inventory with an exact physical and terminal bag, a typed output domain, and a positive/rejected row partition. The parent-owned `tests/task_76_parent_closure.rs` verifies the full classification union, all original AC mappings, 13 dialect identities, three adapter paths, conditional reason/owner/disposition, and required source/terminal/domain/identity evidence. It also independently re-executes native DuckDB direct and nested pagination against the public physical plan, plus DISTINCT and WITH TIES equivalence oracles against a complete input bag.

## Original criteria review

| AC | Mandatory evidence | Reviewed conditional boundary |
| --- | --- | --- |
| #1 Normalize row shaping | TASK-133 LIMIT/OFFSET/FETCH/TOP, TASK-134 WITH TIES/DISTINCT/DISTINCT ON/explicit NULL order, TASK-136 dialect equivalent normalization | QUALIFY placement and engine-specific TABLESAMPLE spellings are represented without claiming unsupported exact combined membership |
| #2 Exact candidate rows and rejections | TASK-133 stable page ordering/NULL/empty cases, TASK-134 peer-group and DISTINCT physical identities, TASK-135 nested and set-branch composition | Underordered peers, implicit NULL sort, ambiguous DISTINCT ON winners yield typed Residual |
| #3 Sampling laws | TASK-136 deterministic row selection is proven separately from stochastic sampling; seed/method/quantity facts remain represented | SYSTEM, BERNOULLI REPEATABLE, SAMPLE and SAMPLE SEED have named TASK-136 Residuals; a seed alone is not an attested algorithm |
| #4 Nested row limits | TASK-135 CTE producer, renamed producer, set branch, NULL duplicate and empty/zero cases with original physical row identities | Unproved peer order or upstream filtered producer remains Residual |
| #5 Native differential proof | Independent DuckDB full physical/result oracles in each child and this parent guard; compiled dbt Core seed/run/docs E2E | Parser compatibility is not native vendor execution |
| #6 Tests, adapters and contract | All 13 exposed dialect parser classifications, direct SQL, compiled dbt and ODCS adapter cells, schema/golden and typed positive/Impossible/Residual assertions from child suites | Native non-DuckDB dialect/session laws remain explicitly uncertified |

The parent does not interpret SQL source-level parsing as an exact physical membership proof. DISTINCT and WITH TIES retain duplicate contributors; row-order assumptions require evidence. An unknown stochastic sample never becomes Exact merely because the parser accepts `SEED`.

## Review and verification

**Standards axis:** Changes are task-owned metadata and an integration test at established public proof seams, with no dependency, source semantics, unsafe code or JSON wire-format change. Each child remains canonical owner of its SQL/data oracles. Parent tests assert existing public analyzer and physical proof behavior plus a separate native DuckDB oracle, without duplicating production logic.

**Spec axis:** The parent inventory maps all six original criteria and tests the complete mandatory/conditional union; original semantic laws are backed by child oracles rather than unverified extrapolation. All conditional vendor/runtime cases retain named owners and explicit dispositions. No release gate is bypassed.

**Final-head gate:** Run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, docs/no-default-feature checks, active schema/golden regression and real dbt Core E2E on this PR head. Do not mark the parent Done until this head is green and the independent review has no blocking gaps. Do not merge automatically; TASK-91 remains independent.
