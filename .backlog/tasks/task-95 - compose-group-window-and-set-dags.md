---
id: TASK-95
title: Compose grouping, ranking and set operators across producer DAGs
status: In Progress
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

**TASK-68 child, execution step 3/8:** Implement this task through independently verified PRs against `main`. PR #95 is the merged baseline increment, not the full task completion. While TASK-95 remains In Progress, it is still the next task under TASK-68's ordered child queue. Do not advance to TASK-96.

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

## Remaining implementation (all five acceptance criteria still open)

1. **JOIN/GROUP composition (#1):** General contributors, grouped-key correlations, aggregate output domains, cardinality and HAVING constraints across multiple interacting materialized operators, not only currently proven templates.
2. **Window composition (#2):** Nontrivial partitions and order correlations through producers, including grouped/joined upstream rows, rank counts, QUALIFY selections and complete physical partitions.
3. **Sets and membership (#3):** General shared-source and interacting branch multiplicities (including UNION ALL per-value distributions, EXCEPT cancellation, zero/negative cases, nested/mixed sets), plus nonempty correlated EXISTS/IN/NOT variants with correct NULL and duplicate semantics.
4. **Derived constraints (#4):** General aggregate-derived predicate inversions, grouping sets and window-derived value constraints where typed producer evidence suffices. Otherwise keep a **typed residual**, without fabricating data.
5. **Sign-off (#5):** Representative feasible, impossible, zero, deliberately rejected and residual proofs across mixed operators; complete cross-dialect parser-boundary and independent DuckDB full-output checks, including NULL/duplicate/empty and overlapping physical-source counterexamples. Update contract, adapters and docs when semantics change.

This is a description of **remaining gaps**, not a mandate to reimplement any of the landed features. Local operator variants belong to TASK-72..75; TASK-95 only composes their proven facts.

## Next-agent handoff

- Select **TASK-95**, not TASK-96 or TASK-68. Preserve the original acceptance criteria; they remain unchecked until independently demonstrated.
- Start a **fresh branch and PR from current `main`**. Do not reopen or extend PR #95, replay its entire diff, or repeat the module split. Use `AGENTS.md`, this task and `docs/code-map.md` to navigate focused modules. Read only the relevant implementation/tests for the chosen unresolved proof.
- Implement and test the remaining TASK-95 scope, building on the landed constructive witnesses. Prefer a complete task sign-off in the next PR if practical; if further reviewed increments are needed, they may merge independently while TASK-95 stays In Progress. No additional child issues are required by this handoff.
- After final verification, mark acceptance criteria #1-#5 Done and TASK-95 Done in the final PR. Only then does the ordered queue advance to TASK-96. TASK-68 stays In Progress until its separate original criteria and all children pass.
