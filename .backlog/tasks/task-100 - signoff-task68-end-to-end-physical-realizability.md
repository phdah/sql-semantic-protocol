---
id: TASK-100
title: Sign off TASK-68 end-to-end physical-source realizability
status: In Progress
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-12'
labels: []
milestone: m-3
dependencies:
  - TASK-99
references:
  - 'TASK-68'
  - 'TASK-66'
  - 'TASK-87'
  - 'TASK-88'
  - 'TASK-89'
  - 'TASK-90'
  - 'TASK-91'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Close the original six TASK-68 acceptance criteria only after verifying the implemented children on main, plus final integration/contract evidence. This child does not replace protocol 3.0.0 TASK-91 release approval.

**TASK-68 child, execution step 8/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Confirm TASK-93..99 are Done and merged, and map every original TASK-68 acceptance criterion #1..#6 to specific source code, tests, schema and documented evidence.
- [x] #2 Audit shared physical identities, multi-parent operator composition, projection inversion, exact positive/negative cardinality, DML states and partial/cyclic/conflicting typed diagnostics; no unsupported shape may be silently feasible.
- [x] #3 Re-run pinned sql-tdg fixture, direct SQL/dbt, cross-dialect canonical, NULL/duplicate and DuckDB complete-DAG oracle tests, including joint terminal and deliberate rejection proofs.
- [x] #4 Verify deterministic Rust API, active JSON schema/examples, adapter parity, coverage manifest, docs and complete CI (fmt, clippy, test, doc, no-default-features and dbt E2E); record checked residual/deferred limitations explicitly.
- [ ] #5 Only then check all TASK-68 parent ACs and mark the parent Done; keep TASK-91 and the protocol 3.0.0 release gate separate and blocked pending their own acceptance.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** Mark TASK-68 Done only when the original six acceptance criteria have independently verified evidence.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.


## Bounded evidence audit (2026-10-12)

The acceptance-to-source/test/schema/doc mapping and typed feasibility audit,
SQL/dbt/DuckDB fixtures, 13-dialect parser-equivalence evidence and
remaining intentionally residual limitations are recorded in
[docs/task-68-signoff.md](../../docs/task-68-signoff.md).

The executable `tests/task_68_signoff.rs` checks completed TASK-93..99 evidence,
guards the still-open TASK-68 and TASK-100 acceptance criteria against
premature closure, and verifies active graph/fixture references. The audited
PR head passed all nine CI jobs, including both dbt E2E projects; CI must
also pass after this correction.

**Blocking TASK-100 AC #5:** The canonical suite still classifies a
positive one-row `mart_customer_summary` outcome from the mixed
join/GROUP/HAVING DAG and a positive `core_enriched` joined output as
`Residual`. These are named in-scope DAG shapes in TASK-68 AC #2/#5.
The concrete DuckDB seeded outcomes are not constructive physical source
plans. Keep TASK-68 In Progress, its original ACs unchecked, and TASK-100
In Progress until those positive plans are proved or the original acceptance
criteria are explicitly revised. The 3.0.0 release and sql-tdg generator
sign-offs remain separate and blocked; no consumer SHA is pinned here.
