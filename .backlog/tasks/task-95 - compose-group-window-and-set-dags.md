---
id: TASK-95
title: Compose grouping, ranking and set operators across producer DAGs
status: Done
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-12'
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

**TASK-68 child, execution step 3/8:** PR #95 supplied the merged baseline. PR #101 completes the canonical mixed-operator physical-source composition requirements against `main`. Once PR #101 lands, the ordered child queue advances to TASK-96; TASK-68 remains In Progress.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Compose JOIN-to-GROUP BY/HAVING with contributor, group-key, aggregate-domain and group-cardinality obligations across materialized producer boundaries.
- [x] #2 Compose window partition/order/rank and QUALIFY selection across producer outputs, with exact rank counts and complete partitions when the local witness proves them.
- [x] #3 Compose UNION/UNION ALL/INTERSECT/EXCEPT branches and supported correlated subquery membership through upstream producers, conserving duplicate and NULL-aware tuple laws.
- [x] #4 Support aggregate-derived filters, grouped-set or window-derived output constraints when the necessary producer proof is available; otherwise emit a typed residual without fabricating values.
- [x] #5 Prove representative positive, zero, rejected and impossible outcomes across mixed operator sequences; validate with cross-dialect and DuckDB full-output differential cases.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** One or more independently verified PRs into `main`, each with code, tests and documentation proportional to its changes. A verified increment may merge while this task remains In Progress. Close only when all five original acceptance criteria are independently satisfied.
- **Next:** After this task is Done, proceed to TASK-96; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.


## Landed baseline: PR #95

[PR #95](https://github.com/phdah/sql-semantic-protocol/pull/95) contains a large **structural, logic-preserving module split** alongside genuine incremental operator-composition work. The line-count change is not a measure of the remaining semantic scope. Its final pre-handoff head `7e5eb5ecf0d308b5aafae086d508bb9e4a102fe2` passed all nine CI jobs (Rust formatting, both lint/test/doc configurations, dependencies, and dbt Core E2E). That CI result validates this landing increment, **not** completion of TASK-95.

Verified or narrowly constructed capabilities added through PR #95:

- Transport exact local GROUP BY/HAVING and ROW_NUMBER()/QUALIFY populations across identity-preserving, type-certified producer chains to writable physical columns, with complete group/rank counts.
- Compose two-branch UNION, UNION ALL, INTERSECT and EXCEPT tuple laws across independently materialized source branches, conserving bag counts and SQL NULL-aware tuple equality. Identical source-column copies can share one complete physical population for proven laws, including even-row UNION ALL outputs.
- Reuse closed operator populations across multiple disjoint physical-source terminal goals; identical shared source counts and transparent goals only where proved. Return typed residuals on unproved source collisions.
- Construct narrow mixed JOIN -> GROUP/HAVING patterns with both single-contributor keys and a complete multiple-contributor matched bag; GROUP -> partitioned rank; grouped COUNT(*) -> simple downstream filters (including rejection); and empty-inner EXISTS/NOT EXISTS/IN/NOT IN through transparent materializations.
- Exercise those supported cases with cross-dialect, DuckDB full-output/NULL/duplicate, negative and fail-closed tests. Unsupported transformations and incomplete metadata stay residual.

## TASK-95 completion: PR #101

[PR #101](https://github.com/phdah/sql-semantic-protocol/pull/101) builds on the verified PR #95 baseline. All five original acceptance criteria are satisfied **for the locally supported exact proof classes**. The composer keeps unsupported, underconstrained, or overlapping populations typed-residual rather than inventing an output or treating derived tables as writable.

1. **JOIN to grouped/HAVING (#1):** Complete matched multi-group key populations model homogeneous repeated contributors, preserving exact SQL bag counts across materializations. Witnesses carry physical `JoinPopulation`, intermediate `Group`, key provenance, non-NULL key `COUNT(column)` versus `COUNT(*)`, aggregate-domain tests and exact surviving group cardinalities. One-to-one and single common-key constructions remain supported.
2. **Ranking and QUALIFY (#2):** New typed `RankPopulation` controls every row in every partition, including strictly distinct ordering, explicit partition sizes and exact `partition_count × min(rows_per_partition, rank_limit)` output counts. Certified transparent producer chains and compatible matched JOIN producers realize these populations at physical leaves; earlier GROUP-to-RANK proofs remain available.
3. **Sets and membership (#3):** UNION/UNION ALL/INTERSECT/EXCEPT tuple laws now lift nested trees with up to four typed branches through independently materialized producers. Repeated physical aliases reconcile their *mapped* column identities and multiplicities before proof, with complete shared-source EXCEPT/EXCEPT ALL cancellation, no filtered-copy substitution and exact NULL-aware tuple equivalence. Correlated equijoin EXISTS/NOT EXISTS and non-NULL IN/NOT IN match and absence proofs use complete physical key populations through materialized sources.
4. **Derived constraints (#4):** Exact grouped COUNT(*) Boolean filters (AND/OR/NOT and both literal-comparison orders), COUNT(non-NULL grouped key) and window-derived rank limits compose from available typed source evidence. Unsupported grouping sets, opaque predicates, unproved numeric transforms and NULL-sensitive alternatives remain typed residuals; no fabricated values or inferred writable intermediate tables.
5. **Independent sign-off (#5):** Added all-supported-dialect parsing/semantic tests, DuckDB full-output checks for positive and rejected memberships, NULLs, duplicates, multi-group JOIN/HAVING, complete ranked partitions, JOIN-to-RANK, nested set-bag algebra and shared-source cancellation. Negative tests cover filtered aliases, noninvertible inputs, source collisions, insufficient evidence and contradictory exact terminal goals (impossible versus residual). The Rust/default-and-minimal-feature lint, test, doc, formatting, dependency, and dbt Core E2E gates passed on implementation commit `1b9f4caef5510e935e43145b7862357e79823314` ([CI run 38259582643](https://github.com/phdah/sql-semantic-protocol/actions/runs/38259582643)). Later commits update documentation/backlog only.

**Scope boundary:** This is complete for TASK-95's **composition of authoritative local witness laws**, not arbitrary SQL inference. Other TASK-70..91 tickets own additional local operator variants. TASK-96 owns projection inversion/output distributions; subsequent TASK-97..100 and parent TASK-68 still own their separate release gates. The canonical source-independent DAG, value-domain contract, adapter parity, and explicit residual behavior remain intact.

**Queue:** TASK-95 Done; TASK-96 next (after PR #101 merges). Parent TASK-68 stays In Progress.
