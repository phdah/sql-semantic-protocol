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
- [x] #1 Pin and enumerate exact committed sql-tdg multi-layer, join/group/window/set and scripted DML/DDL fixture sources, keeping a reproducible upstream SHA and shape inventory.
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


## Pinned executable certification evidence (2026-10-12)

The PR pins sql-tdg revision `90ec0e12a2d5cd2c1294556336fc947e176c9db9`
under `tests/fixtures/sql_tdg/`, including all seven committed raw SQL
fixtures, all eleven dbt model SQL files and four dbt seed relations. The
pinned corpus is exercised by `tests/sql_tdg_physical.rs` and the independent
`tests/sql_tdg_pinned_dbt_e2e.rs` DuckDB/dbt integration gate invoked by
`make dbt-e2e`. Exact physical proof and residual boundaries are recorded
in `docs/tdg-certification.md` and the executable coverage manifest.

**Closure guard:** Do not convert a DuckDB example with a residual upstream
physical plan into a general constructive certificate. The original pinned
upstream revision does not commit UPDATE/DELETE/MERGE/CTAS scripts; those
operator-level source-state laws are tested here, while downstream generated
script coverage remains owned by sql-tdg TASK-31/36 and protocol TASK-91.

## Final TASK-99 proof handoff

All five TASK-99 criteria are covered by PR #105. The pinned upstream
commit is the sole SQL/dbt fixture reference. The concrete dbt pipeline
seeds all four source tables once and checks exact row counts and value
snapshots for each of eleven terminals, including deliberate rejection and
bag duplicates; the upstream insert script is checked against an exact
typed before/after physical state. Cross-dialect canonical operator outcomes
are checked by `tests/sql_tdg_dialect_laws.rs` wherever the parser accepts
the equivalent form. `tests/sql_tdg_pinned_dbt_e2e.rs` checks source-to-model
graph identity, direct SQL versus dbt adapter zero-row proof parity, typed
positive residuals and exact contradictions. Normal, no-default-feature and
dbt Core CI are required gates.

**Deliberately not certified:** general positive joined/grouped/ranked
generation with incomplete upstream physical construction, arbitrary
correlated EXISTS source populations, absent upstream scripted
UPDATE/DELETE/MERGE/CTAS fixture files, and sql-tdg's own randomized
multi-seed rejection/output snapshots. These are explicitly residual or
downstream-owned by TASK-100, TASK-89/91 and sql-tdg TASK-31/35/36, not
silent feasibility claims. TASK-68 remains In Progress and TASK-100 is next.
