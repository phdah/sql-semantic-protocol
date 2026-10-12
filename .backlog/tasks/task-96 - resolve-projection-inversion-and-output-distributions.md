---
id: TASK-96
title: Prove safe projection inversion and complete output distributions
status: Done
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
- [x] #1 Compose renamed, reordered and selectively projected columns over multiple producers using canonical typed lineage; require complete schema and reference evidence for physical leaves.
- [x] #2 Invert proven injective/reversible expressions and alias mappings with explicit overflow, NULL and type guards; leave lossy, noninvertible or ambiguous expressions as typed residual.
- [x] #3 Construct complete physical-source assignments for requested exact terminal value distributions, including frequencies, duplicates, NULLs, and count reconciliation across correlated terminals.
- [x] #4 Preserve value-domain bounds, inclusivity, constraints and proof strength through producer and branch composition; never treat a distribution at an intermediate boundary as directly writable source data.
- [x] #5 Test reversible and non-reversible transformations, contradictory distributions, computed projections and cross-dialect dbt/SQL metadata parity against DuckDB output histograms.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-97; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.


## Completion evidence: PR #102

[PR #102](https://github.com/phdah/sql-semantic-protocol/pull/102) completes
the five original TASK-96 criteria for **locally certified, complete
physical-source distribution proof classes**. This is a typed extension of
the existing source-independent physical DAG; SQL or dbt consumers never
infer missing transformations or write intermediate producer relations.

1. **Typed lineage and projection (#1):** `physical_realization/distributions.rs`
   resolves renamed, reordered and selectively projected outputs across
   catalog-attested materializations. Every producer edge needs the complete
   canonical relation, column identity, exactly one producer, unchanged
   datatype, and a fully row-preserving operation. Missing or ambiguous
   evidence retains residual status.
2. **Safe inversion (#2):** Checked, forward-verified signed integer unary
   negation, addition, and subtraction compose over successive producer
   boundaries. Type widths, SQL NULL propagation, overflow, competing alias
   frequencies, noninvertible computations and uncovered projected
   expressions are guarded. Direct copied bool and variable-width text
   histograms also validate physical datatype and length evidence.
3. **Complete distributions and joint terminals (#3):**
   `physical_joint_distribution_plan` constructs typed closed-world
   `source_rows` operator populations and terminal row bounds over shared
   or independent physical inputs. Frequency totals, duplicates, NULL
   values, alternate aliases, distinct source columns, count-only terminal
   goals and joint zero states are reconciled deterministically. Necessary
   conflicting complete bags and source counts are impossible; unsupported
   correlated populations stay residual.
4. **Domains and branch composition (#4):** Canonical value-domain
   membership checks distinguish open/closed range bounds, finite sets and
   unknown comparisons. Each computed producer output is checked against
   those domains and materialization constraints before inversion. Reversible
   output histograms downstream of supported UNION/UNION ALL/INTERSECT/EXCEPT
   set DAGs reuse the existing physical tuple-population witness, preserving
   exact SQL NULL and duplicate laws and rejecting mismatched physical versus
   materialized output types. No intermediate data becomes writable.
5. **Independent validation (#5):** Cross-dialect tests cover reversible
   and non-reversible projections, impossible alias frequencies, overflow,
   source/producer schema mismatches, typed/null/duplicate histograms,
   independent and shared terminal counts, and set branch values.
   DuckDB oracle tests execute complete physical source inserts and output
   histograms. `tests/dbt_manifest.rs` compares direct SQL versus compiled
   dbt manifest/catalog source histogram assignments. Existing active
   `outcomeWitness` schema and emission remain unchanged.
   [CI run 38283000490](https://github.com/phdah/sql-semantic-protocol/actions/runs/38283000490)
   passed all nine jobs: fmt, lint, tests, docs, no-default-features,
   dependency check, and dbt Core E2E.

**Audited residual boundaries:** Filtered, grouped and ranked distributions
without a canonical complete population, multi-join correlations, nonlinear
or lossy expressions, uncertain datatype conversion, missing metadata,
contradictory or opaque source constraints, and partial/cyclic producers do
not receive a feasible proof. Additional local operator variants and
complete generator sign-off belong to TASK-70..91, TASK-97..100 or parent
TASK-68 rather than silently expanding TASK-96 claims.

**Queue:** TASK-96 Done; merge PR #102 before taking TASK-97, the next
child of TASK-68. Parent TASK-68 remains In Progress; protocol 3.0.0
release and sql-tdg sign-off remain blocked separately.
