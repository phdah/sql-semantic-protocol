# TASK-113 acceptance audit

Status: **implementation under final verification**. Do not mark Done or merge the draft until final-head CI and the assertions below have been reviewed.

## Scope

- Owned frozen inventory: `docs/task-113-acceptance-freeze.json`, version 1, stable fixtures `71-C113-P` and `71-C113-N`.
- Exactly seven named forms: CROSS, USING, NATURAL, OR-on, non-equi, IS NOT DISTINCT FROM and null-safe equality.
- Fourteen independently enumerated complete physical row bags, full terminal bags and exhaustive NOT TRUE pair partitions. Each bag fixes SQL NULL membership and typed inclusive output extrema.
- Thirteen project dialects per case (182 observed parser/canonical cells). Native execution claims apply only to DuckDB. The vendor `<=>` spelling is normalized to `IS NOT DISTINCT FROM` for the DuckDB oracle; no vendor-native equality/collation law is certified.

## Model evidence

- `src/analysis/joins.rs` maps NATURAL to exactly the known visible column intersection, retains typed unknown-schema diagnostics and correctly normalizes a known empty intersection to CROSS.
- `src/physical_realization/join_uniform.rs` emits uniform, complete, typed and closed source-bag constructions for Cartesian, signed INTEGER less-than, non-NULL null-safe equality and a restricted, proven equality-OR-left-NULL form. Each key assignment is checked against authoritative physical schema ranges; all other expressions and missing evidence remain residual.
- Ordinary USING and schema-resolved NATURAL equality reuse the existing canonical equality-bag solver. A `UniformJoinPopulation` obligation states the exact source identities, key values, SQL pair law and source/output multiplicities and is checked independently in `src/constructive/cases.rs` and emitted through the active protocol schema.
- No arbitrary OR disjunction, computed join key, absent schema, string collation or vendor-specific equality/session law is elevated to Exact.

## Independent tests

- `tests/task_113_portable_joins.rs`: each named P/N case executes full physical DuckDB bags, positive output identity bags, NULL, inclusive numeric bounds and complete `IS NOT TRUE` complements; public physical plans must prove the same finite output count with two closed source leaves.
- `tests/task_113_parser_matrix.rs`: all 182 recorded dialect/parser normalizations compared against canonical protocol AST forms; a parser result alone is not a native-engine certification.
- `tests/task_113_join_adapters.rs`: all seven forms retain one equivalent physical proof through direct SQL, compiled dbt manifest and ODCS metadata when schemas are equivalent.
- Historical regression expectations upgraded only where TASK-113 now proves a law. Computed keys, arbitrary OR and text collation remain conservative residuals.

## Conditional release boundaries

- `J113-VENDOR-COLLATION`: a trusted comparison/collation or session law is required; owner TASK-113, pending maintainer review, no native certification.
- `J113-NATURAL-UNKNOWN-SCHEMA`: complete unambiguous column names are required on both visible inputs; unsupported NATURAL condition stays residual, owner TASK-113, pending reviewed release disposition.
- `J113-NULLSAFE-PARSER`: parser equivalence alone does not attest native vendor `<=>` semantics, collation or session settings; owner TASK-113, pending reviewed release disposition.

## Final-head sign-off gate

Before Done, verify focused test assertions against all 14 frozen cases, all 182 dialect cells and all adapter facts. Require passing Rust formatting, strict lint, default and no-default tests/docs, dependency checks, dbt E2E and active protocol/schema snapshots on the last head commit. Record workflow and commit here; leave TASK-71 open after this child.
