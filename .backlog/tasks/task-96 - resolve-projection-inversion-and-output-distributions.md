---
id: TASK-96
title: Prove safe projection inversion and complete output distributions
status: In Progress
assignee: []
created_date: '2026-10-10'
updated_date: '2026-10-12'
labels: []
milestone: m-3
dependencies:
  - TASK-95
references:
  - 'TASK-68'
  - 'TASK-70'
  - 'TASK-79'
  - 'TASK-85'
  - 'TASK-87'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Resolve output value obligations back to canonical physical columns and expressions while retaining exactness. This is physical DAG inversion, not a second implementation of expression analysis or outcome histograms.

**TASK-68 child, execution step 4/8:** After foundational PR #92 is merged, implement this task in its own PR against `main`. Follow the ordered child queue declared in TASK-68; do not work around a blocked prerequisite by marking it Done.

**Release contract:** Preserve the canonical source-independent relational DAG and typed source-level constructive proof. Reuse upstream operator-local semantics and their authoritative typed evidence; do not duplicate them here or reparse SQL in sql-tdg. Prove feasible versus impossible versus residual soundly, preserve strongest justified outcome domains, and keep release-blocking coverage unverified until independently certified.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Compose renamed, reordered and selectively projected columns over multiple producers using canonical typed lineage; require complete schema and reference evidence for physical leaves.
- [ ] #2 Invert proven injective/reversible expressions and alias mappings with explicit overflow, NULL and type guards; leave lossy, noninvertible or ambiguous expressions as typed residual.
- [ ] #3 Construct complete physical-source assignments for requested exact terminal value distributions, including frequencies, duplicates, NULLs, and count reconciliation across correlated terminals.
- [ ] #4 Preserve value-domain bounds, inclusivity, constraints and proof strength through producer and branch composition; never treat a distribution at an intermediate boundary as directly writable source data.
- [ ] #5 Test reversible and non-reversible transformations, contradictory distributions, computed projections and cross-dialect dbt/SQL metadata parity against DuckDB output histograms.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-97; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.


## Implementation progress: physical histogram inversion PR

- Extracted histogram composition into a canonical physical-distribution owner while retaining the existing `source_rows` protocol shape.
- Reconciled repeated output aliases to one physical source column, rejected contradictory direct-copy histograms, and verified complete count totals.
- Added guarded inverse mappings of reversible signed integer terminal expressions over a typed transparent materialization chain; reject arithmetic overflow, SQL NULL mispropagation, lossy projections, datatype mismatch, partial producers, and unchecked expressions.
- Added cross-dialect, NULL/duplicate/overflow cases and complete DuckDB output-histogram checks.

**Additional implementation:** deterministic joint terminal histograms across independent and shared physical sources, typed SourceRows operator populations with closed-world count witnesses, nested materialized integer inversions, boolean and VARCHAR source frequencies, canonical interval-bound/inclusivity validation, and dbt catalog versus raw-SQL source-histogram parity. Missing expression, row-shape and metadata proofs remain residual.

**Remaining for closure:** multi-parent/branch distribution inversion beyond already-certified set laws, richer semantic domain propagation and proof-strength evidence, full cross-feature and dbt oracle audit. Task stays open until verified. All original acceptance criteria remain open until those requirements are discharged and CI evidence is available. TASK-97 stays blocked.
