---
id: TASK-95
title: Compose grouping, ranking and set operators across producer DAGs
status: In Progress
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-10'
labels: []
milestone: m-3
dependencies:
  - TASK-94
references:
  - 'TASK-68'
  - 'TASK-72'
  - 'TASK-73'
  - 'TASK-74'
  - 'TASK-75'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Realize cross-layer compositions of already-proven local group, window, set and subquery facts. Local variants remain owned by TASK-72..75; this child owns lifting their exact proof obligations to one physical-source plan.

**TASK-68 child, execution step 3/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Compose JOIN-to-GROUP BY/HAVING with contributor, group-key, aggregate-domain and group-cardinality obligations across materialized producer boundaries.
- [ ] #2 Compose window partition/order/rank and QUALIFY selection across producer outputs, with exact rank counts and complete partitions when the local witness proves them.
- [ ] #3 Compose UNION/UNION ALL/INTERSECT/EXCEPT branches and supported correlated subquery membership through upstream producers, conserving duplicate and NULL-aware tuple laws.
- [ ] #4 Support aggregate-derived filters, grouped-set or window-derived output constraints when the necessary producer proof is available; otherwise emit a typed residual without fabricating values.
- [ ] #5 Prove representative positive, zero, rejected and impossible outcomes across mixed operator sequences; validate with cross-dialect and DuckDB full-output differential cases.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-96; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.


## Implementation progress (draft PR)

- Lifted exact grouped COUNT(*) / HAVING and ROW_NUMBER() / QUALIFY source-population
  witnesses through several row-preserving producer layers, proving typed
  source-column lineage at every intermediate schema boundary.
- Remapped group, partition and ordering keys to the actual writable physical
  source, preserving the pre-existing local operator law and full input count.
  Noninvertible projections, filters, unrelated operators, incomplete schemas
  and shared-source mixed plans continue to return residual.
- Composed exact two-branch UNION/UNION ALL/INTERSECT/EXCEPT tuple multiplicity
  through independently materialized identity-only branch producers, mapping
  full candidate-tuple laws to schema-certified physical source columns.
- Added DuckDB complete-output and NULL-duplicate differential checks,
  cross-dialect grouping tests and fail-closed negative producer cases.

**Additional physical DAG progress:** a canonical schema-backed `OperatorPopulation`
obligation captures complete grouped, ranked and set source fixtures. The
joint solver composes disjoint operator populations, identical shared-source
populations and exact transparent source counts, with explicit residuals for
unproved shared assignment collisions. Two mixed chains have narrow
constructive proofs: distinct one-to-one equijoin -> group with one
contributor per key, and a grouped producer -> one-row-per-partition QUALIFY.

**Still open:** general join-to-group with nontrivial contributors,
join/group/window/set mixed branches, different-predicate and correlated
shared-source plans, aggregate-derived filters, grouped sets, correlated
subqueries, negative/rejected mixed outcomes, further NULL/duplicate
counterexamples and full cross-dialect sign-off. Acceptance remains open.

### Additional implementation and CI verification
- A common matched equijoin bag now realizes one GROUP BY/HAVING group
  with multiple physical contributors, retaining typed join and local group
  obligations on the exact producer boundary.
- Joint mixed plans reuse independently closed physical source populations
  for transparent terminal counts when all physical counts agree, otherwise
  emit typed residual.
- A grouped producer's exact COUNT(*) output supports simple downstream
  row predicates, with nonempty closed-world negative/rejected outcomes
  and DuckDB positive/negative differential oracles.
- Exact correlated/uncorrelated `EXISTS`, `NOT EXISTS`, `IN` and
  `NOT IN` NoCandidates cases can be physically realized through
  row-preserving producer chains with an empty inner source. NULL-sensitive
  behavior is exercised with actual DuckDB inputs.
- The full CI matrix passed at commit `a195e29ce83ea007c847c01dd732f848beaa79a4`
  (format, lint, tests, docs, no-default-features, and dbt Core E2E).

**Outstanding:** General correlated nonempty membership, arbitrary
mixed grouping/window/set DAGs, computed aggregate inversions, shared
source set cases, mutually constraining different operator populations,
complete impossible-versus-residual law proofs and acceptance-wide
cross-dialect sign-off. TASK-95's five criteria remain unchecked.
