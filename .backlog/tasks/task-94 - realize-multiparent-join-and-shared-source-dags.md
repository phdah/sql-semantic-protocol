---
id: TASK-94
title: Realize multi-parent joins and repeated-source DAGs
status: To Do
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-10'
labels: []
milestone: m-3
dependencies:
  - TASK-93
references:
  - 'TASK-68'
  - 'TASK-69'
  - 'TASK-71'
  - 'TASK-77'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Compose already-proven join multiplicity and membership facts through several independently materialized and shared producer parents, without treating intermediate relations as independently writable sources. TASK-71 remains owner of missing join-local variants.

**TASK-68 child, execution step 2/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Compose canonical equi, outer, semi, anti, cross and supported many-to-many join evidence through independent multi-layer parent producers where local operators carry sufficient proof.
- [ ] #2 Reconcile self-joins, aliases, repeated producers and overlapping physical sources with shared row identities; model multiplicity and NULL extensions without falsely independent key assignments.
- [ ] #3 Construct compatible nonzero and zero terminal counts, joining and rejected-row witnesses with complete physical source assignments and necessary absence/no-partner evidence.
- [ ] #4 Conserve exact lineage, key types, value domains, cardinality laws and materialization/write-kind boundaries; report unsupported join topology or conflicting constraints explicitly.
- [ ] #5 Add unit, all applicable dialect parser-boundary checks and DuckDB E2E tests for joined materialized parents, unmatched rows, NULL keys, duplicates, impossible/residual shared-source cases.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-95; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.

