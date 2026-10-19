# TASK-121 independent bounded acceptance audit

Date: 2026-10-19
Owner: TASK-121, child 1/4 of unfinished TASK-73
Acceptance freeze: [version 1](task-121-acceptance-freeze.json)
Review base: `main`
Reviewed semantic head: `dfd50662aa0fec3d45ca3fcd3cf7b427c9746fab`
CI: [9/9 jobs passed](https://github.com/phdah/sql-semantic-protocol/actions/runs/39267305885)

## Frozen mandatory cells and public source proof

The finite v1 matrix contains five explicit mandatory cases over physically typed `s(id VARCHAR, k INTEGER, x INTEGER)`, with both `73-C121-P` and `73-C121-N` fixture families. `tests/task_121_aggregate_bags.rs` compares independent complete DuckDB physical source snapshots and terminal SQL bags to the frozen expected source and output row sets, including output domains and SQL NULL membership.

1. `73-C121-P-global`: input x values [1,1,NULL] proves exactly one global result, COUNT(*)=3, COUNT(x)=2, SUM=2, MIN=1, MAX=1, AVG=1, retaining all three physical source identities.
2. `73-C121-P-grouped`: input k values [1,1,2] yields two distinct complete groups; for k=2 COUNT(*)=1, COUNT(x)=0, other four aggregate outputs SQL NULL.
3. `73-C121-N-global-empty`: empty complete input yields exactly one terminal group with both COUNTs zero and SUM/MIN/MAX/AVG SQL NULL.
4. `73-C121-N-grouped-empty`: empty complete input yields no grouped rows; no invented global group.
5. `73-C121-N-null-only`: one NULL x row contributes to COUNT(*) but not COUNT(x), SUM, MIN, MAX or AVG. The NULL result value domains are exact.

All five are source-backed Exact, not parser-only lineage or guesswork. The public source-plan interpreter `src/physical_realization/aggregate_bags.rs` uses canonical parsed aggregate expressions and schema-attested complete input rows and returns exact contributor identities, full terminal bags and exact finite `ValueDomain` sets. Integer aggregation accumulates through checked i128 operations, never unchecked overflow.

## Acceptance criteria reviewed

- **#1 Finite matrix:** five named mandatory cases plus three reviewed conditional typed/runtime boundaries are frozen and registered in `docs/coverage-manifest.json`, with exact SQL, canonical source identities, NULL-inclusive finite domains, full expected rows, 13 canonical dialects, 3 adapters, owner and explicit native-runtime disposition. No mandatory case is classified as Residual.
- **#2 Canonical physical semantics:** all six basic aggregates are exact for the bounded signed-integer oracle. Complete source multiplicity, grouped/global emptiness, NULL noncontribution and source IDs are proven together. Duplicate complete physical relation identity is Impossible, while absent evidence is Residual. Nonintegral AVG lacks an attested scale/rounding law and is explicitly Residual, not incorrectly truncated to an integer.
- **#3 Independent executable oracles:** five public API source and terminal complete-bag comparisons with DuckDB, plus an explicit Impossible/Residual regression. Values and domains are checked independently of the returned physical proof.
- **#4 Dialects and adapters:** 5 x 13 = 65 canonical dialect parser/physical exact bag cells, plus five `postgres` alias cells (70 paths in CI). 5 x 3 = 15 direct SQL, compiled dbt and ODCS canonical source and aggregate proof parity checks. These certify portable parsing and schema provenance only; no native vendor-engine execution is asserted.
- **#5 Final verification:** all nine CI jobs passed at `dfd50662aa0fec3d45ca3fcd3cf7b427c9746fab`: formatting, strict Clippy, full and no-default Rust tests, docs, no-default dependencies and dbt Core E2E. The task metadata commit requires an additional last-head CI run before merge. TASK-73 remains open.

## Two-axis implementation review

**Standards:** New proof is in its single canonical physical realization owner; no SQL reparsing, unchecked host arithmetic or silent positive assertions; exported API, code map, public Rust seam and fixtures agree. Strict lint and formatting passed.

**Specification:** Every frozen mandatory case has an independent source/terminal bag, 13-dialect classification and the three representable adapter proofs. Conditional decimal/nonintegral AVG, integer overflow and noninteger/collated values require additional authoritative type or session laws and retain explicit TASK-121 ownership; they are not claimed exact. HAVING, DISTINCT/FILTER and join-fed grouping belong to subsequent TASK-122/123 and were deliberately not implemented in this child. Parent TASK-73 and release TASK-91 are not signed off.

## Merge gate

Run CI after this metadata-only sign-off change; confirm all nine jobs on the new head before merging. The next eligible child is TASK-122 only after this PR merges.
