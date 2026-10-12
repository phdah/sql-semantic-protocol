---
id: TASK-99
title: Certify physical-source proofs against committed sql-tdg fixture shapes
status: Done
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-12'
labels: []
milestone: m-3
dependencies:
  - TASK-98
references:
  - 'TASK-68'
  - 'TASK-88'
  - 'TASK-89'
  - 'TASK-91'
  - 'sql-tdg TASK-24..36'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Certify actual pinned sql-tdg raw-SQL/dbt fixture shapes with the canonical protocol, without shifting semantic inference into sql-tdg. This is TASK-68 producer-plan integration; TASK-89/91 own broader final cross-feature and release sign-off.

**TASK-68 child, execution step 7/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Adopt and enumerate multi-layer, join/group/window/set and scripted DML/DDL fixture shapes as a protocol-owned versioned conformance suite, reproducible by protocol Git SHA; retain the original donor SHA only as historical provenance.
- [x] #2 Produce deterministic source-level constructive plans for all in-scope feasible fixture outcomes and typed impossible/residual diagnostics for unproved shapes, without consumer-side SQL reparsing.
- [x] #3 Run complete DuckDB pipeline oracles with physically inserted input rows; verify all jointly targeted terminal counts, distributions and deliberate rejection/nonmembership, including NULLs and duplicates.
- [x] #4 Compare canonical semantics across all supported dialects where sqlparser accepts equivalent fixture forms and check raw SQL versus dbt artifact/catalog adapter parity.
- [x] #5 Update proof evidence links and cross-repository dependency mapping in coverage manifest and TASK-91 without marking generator TASK-35/36 or protocol 3.0.0 release accepted prematurely.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-100; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.


## Canonical conformance certification (2026-10-12)

The protocol **owns** `tests/fixtures/conformance/v1/`, including seven raw SQL
scripts, eleven dbt models, four physical source seeds and the portable
machine-readable `suite.json` with expected outcomes. The initial examples
came from sql-tdg commit `90ec0e12a2d5cd2c1294556336fc947e176c9db9`,
but this is historical provenance. There is no consumer repo pin or import;
consumers pin a **protocol** Git SHA or published release.

`tests/protocol_conformance_physical.rs` checks sound source construction,
NULLs, duplicates, rejection and ordered INSERT state;
`tests/protocol_conformance_dialect_laws.rs` checks shared canonical meaning.
`make dbt-e2e` runs the canonical dbt project and
`tests/protocol_conformance_dbt_e2e.rs` checks the eleven outputs against
`suite.json` row counts and value snapshots, source identity, adapter parity
and typed residuals/contradictions.

**Closure guard:** Successful parsing and executable DuckDB examples do not
prove arbitrary constructive physical-source feasibility. General joined,
grouped, ranked or correlated outputs and randomized downstream generator
fixtures remain residual or downstream-owned by sql-tdg TASK-31/35/36 and
protocol TASK-89/91. TASK-68 remains In Progress; TASK-100 is next.
