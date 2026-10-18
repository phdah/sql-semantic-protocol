# TASK-116 independent TASK-71 parent acceptance audit

**Status: all five bounded TASK-116 acceptance criteria verified.** Independent
review of original TASK-71 AC #1..#6 is complete for portable certified cases,
but TASK-71 stays In Progress until this PR merges. No native vendor-engine execution
or TASK-91 release approval is implied by this audit.

## Frozen independent verification boundary

- [docs/task-116-acceptance-freeze.json](task-116-acceptance-freeze.json)
  version 1 freezes **15 mandatory** and **12 conditional** original
  `J71-*` cases, under immutable `71-C116-P` and `71-C116-N` fixtures.
- [tests/task_116_parent_join_oracles.rs](../tests/task_116_parent_join_oracles.rs)
  independently seeds the complete physical sources for every mandatory
  original cell, executes the original SQL (with DuckDB's SEMI/ANTI spelling),
  checks the entire terminal multiset with integer/NULL domain extrema, and
  checks individual source identities. Negative ON cases partition the entire
  physical Cartesian source bag into matching and NOT TRUE pairs.
- This includes two- and three-way duplicate bags, composite keys, aliased
  self-join, unmatched LEFT/RIGHT/FULL, SEMI/ANTI closed-world absence,
  reversed operand order, NULL and disjoint rejected joins, post-WHERE chained
  joins and an enforced physical FK case. The separate parent conditional
  `J71-OUTER-WHERE` regression proves WHERE is evaluated *after* synthetic
  outer NULL-extension, without fabricating a right source.
- Parent evidence is intentionally **independent** of the fixture values used
  in TASK-109..115. It does not compute expected output by running protocol
  analysis or reusing the children's expected bags.

## Original parent AC reconciliation

| Parent AC | Independent parent evidence | Child implementation proof | Status |
| --- | --- | --- | --- |
| 1: join families, alias, cardinalities | 15 mandatory whole-bag oracle cells; 12 conditional rows | TASK-109..113, TASK-115 finite closures | Independently verified in CI run 39155624537 |
| 2: ASOF/range/time, LATERAL/APPLY | 4 named conditional cells with explicit vendor runtime boundaries | TASK-114 portable temporal/correlation proof and parser matrix | Portable evidence verified; native uncertified |
| 3: physical IDs and nonmatching/null/rejected | Complete source IDs, terminal identities and NOT TRUE partitions for parent bags; outer WHERE regression | TASK-109..115 closed-world construction and independent DuckDB | Independently verified in CI run 39155624537 |
| 4: filters, keys, FK, duplicates and cardinality | 3-way WHERE; composite; physically enforced FK; self/many-to-many | TASK-115 full finite chain/domain and constraint evidence | Independently verified in CI run 39155624537 |
| 5: execution | DuckDB source+terminal bag evidence plus child source/oracle suites | TASK-109..115 DuckDB differential oracles | Portable DuckDB verified; native vendor engines not attested |
| 6: dialect, adapters, tests, schema, docs | 13 distinct dialect identities; `postgres` is the alias of `postgresql` (14 accepted spellings). Full CI includes child public/API oracle, dbt, ODCS, no-default, schema tests | TASK-109..115 adapter/dialect suites and canonical emission tests | Independently verified in CI run 39155624537 |

The parent-level matrix preserves the original TASK-71 variant spelling and
expected terminal counts. Conditional evidence points to a **specific** child
fixture or an independently executed TASK-116 oracle. No parent cell claims
native vendor execution on the strength of an sqlparser parse or DuckDB alone.

## Verified CI and remaining parent release gates

1. Independent DuckDB checks pass for all 15 mandatory parent cases, including source/row multiplicities, bounds, NULL and negative/absence evidence. The six original parent ACs have been reconciled with the 12 parent-owned conditional cells.
2. GitHub CI [run 39155624537](https://github.com/phdah/sql-semantic-protocol/actions/runs/39155624537) succeeded in all nine jobs: formatting, both strict Clippy configurations, default and no-default tests, docs, dependency check and dbt E2E.
3. Native vendor/session execution is still **not certified**. Every conditional cell retains its named evidence gap and owner TASK-116, and the v3 release is not approved on parser/DuckDB evidence alone.
4. TASK-71 remains In Progress until TASK-116's PR merges and a separate parent task closure is recorded. TASK-91 and v3 release remain independent gates.

## Implementation provenance

The single authoritative parent inventory remains
[docs/task-71-acceptance-freeze.json](task-71-acceptance-freeze.json).
The TASK-116 matrix adds independent source/terminal concrete values and
provenance per parent cell without weakening any parent acceptance criterion.
The [coverage manifest](coverage-manifest.json) links both contracts.
