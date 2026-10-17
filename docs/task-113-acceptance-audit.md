# TASK-113 acceptance audit

Status: **bounded TASK-113 acceptance verified** on implementation commit `3f537cfc789c38b9c2b47c7db84e209ecbff9c43`; final signoff-metadata CI must pass before merge. TASK-71 remains In Progress.

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

## Independent final AC review (2026-10-17)

All **five child ACs** have been checked against the frozen matrix, the canonical implementation and independent public-API/execution assertions. This is a TASK-113-only certification, not TASK-71 or 3.0.0 release approval.

| Child AC | Verification and evidence | Signoff |
| --- | --- | --- |
| #1 Frozen scope | 14/14 P/N cases for seven named forms, stable two-source row/column IDs, typed INTEGER/VARCHAR source facts, output NULL/extrema/inclusive bounds, complete expected and NOT TRUE bags, and 182/182 versioned parser/canonical classifications. Linked in coverage manifest. | Pass |
| #2 Canonical implementation | Canonical schema-backed NATURAL/USING equality or typed unknown-schema diagnostic; CROSS and restricted OR, signed non-equi, IS NOT DISTINCT FROM and null-safe source-population laws with exact signed INTEGER key assignments. Checked source cardinalities and complete physical leaves for each frozen target count; no consumer SQL reparsing. | Pass |
| #3 Independent physical oracle | Each of the 14 complete input bags is inserted into DuckDB; the full terminal row-identity bag, count, NULL, numeric bounds and complete NOT TRUE complement are asserted. The final regression also explicitly checks that TRUE and NOT TRUE bags partition every Cartesian physical source pair with multiplicity. | Pass |
| #4 Dialects, parity, conditional laws | 182/182 cells exercised across 13 project dialects; adapter test exercises all seven forms through direct SQL, compiled dbt and equivalent ODCS schemas. NATURAL without complete columns yields typed Residual, with explicit warning; vendor collation and native `<=>` execution remain owned conditional/pending-release-review limits, not certified. | Pass |
| #5 Full verification | 9/9 CI jobs passed on implementation commit: rustfmt, strict all-target/all-feature Clippy, full Rust tests (including schema/snapshot), docs, no-default-features lint/test/doc/dependencies, and dbt Core E2E. See run below. Final metadata-only HEAD must also pass before merge. | Pass |

**Implementation head:** `3f537cfc789c38b9c2b47c7db84e209ecbff9c43`.

**Verified CI:** https://github.com/phdah/sql-semantic-protocol/actions/runs/39052136373 (all nine jobs successful).

**Reviewed proof limitation:** The existential uniform-key physical population proves a complete alternative source bag for the requested cardinality, not necessarily identical literal rows to the frozen DuckDB source bag. The literal nullable SQL oracle separately certifies the frozen row identities/NULL/output extrema. No claim is made that the alternative non-NULL integer witness physically inserts NULL or attests a vendor session/collation law. The three named conditional boundaries retain owner TASK-113 and `pending_release_review`.

**Handoff:** TASK-113 Done in PR #122, awaiting merge. TASK-71 remains In Progress, TASK-114 is blocked until #122 merges, and no protocol-3.0.0 release approval is implied.
