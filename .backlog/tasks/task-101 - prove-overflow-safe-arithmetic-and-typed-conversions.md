---
id: TASK-101
title: Prove typed arithmetic, overflow and conversion outcomes
status: In Progress
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-14'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-79
references:
  - 'TASK-70'
  - 'TASK-88'
  - 'TASK-90'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Complete the canonical typed arithmetic and conversion families needed by predicates and expressions. Extend the already verified integer offset and widened signed cast subset; keep runtime-dependent or unbounded conversions residual until their laws are independently attested.

**TASK-70 child, step 1/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize source-independent unary `+`/`-`, typed binary `+`/`-`/`*`/`/`/`%` and deterministic literal coercion with explicit type-width, precision, SQL NULL, division-by-zero and overflow/underflow guards. Handle the soundly provable integer/decimal families and emit typed residual for every unsupported operator/type pair.
- [ ] #2 Produce strongest safely provable output domains (bounds, inclusivity, finite sets and excluded values) and reversible source constraints; certify inversion only when total/injective over the attested source domain, including signed boundaries and noninvertible or multiply-valued operations.
- [ ] #3 Handle CAST, TRY_CAST and SAFE_CAST where parsing, type evidence and failure behavior are certified; cover identity/widening/narrowing and exact decimal or text conversions only under demonstrated semantic laws. Distinguish exception, NULL-on-failure and implicit/session-dependent coercions without silently claiming exactness.
- [ ] #4 Prove TRUE and NOT TRUE (FALSE or UNKNOWN) outcomes, impossibility, and residual status for mixed arithmetic/comparison cases; do not infer global absence from finite sampling or unchecked arithmetic.
- [ ] #5 Add direct SQL and dbt/catalog parity, all parser-capable dialect equivalence tests, DuckDB executable arithmetic/NULL/overflow/zero divisors and positive/negative/impossible/residual oracles. Update canonical Rust API, schema, semantic docs and capability evidence for each claimed family.
<!-- AC:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Arithmetic, coercion and their row-local semantic facts only. TASK-102 owns branch expressions; TASK-103 owns correlated Boolean proof; TASK-106 owns relational/physical composition. TASK-88 still owns global dialect-law certification.
- **Next:** After completion, pick **TASK-102**.

## Implementation progress (2026-10-14)

The branch now contains a broader, bounded proof implementation in
`src/boolean_witness/numeric.rs`, `src/analysis/arithmetic_domains.rs`,
`src/analysis/decimal_literals.rs` and `src/analysis/cast_domains.rs`:

- Catalog-certified signed affine and correlated cancellation/arithmetic; exact
  finite sets, enumerable disjoint bounded ranges, conservative interval bounds
  and invertible exclusions.
- Exact integral integer division within binary64-exact limits; nonnegative
  positive-divisor remainder outputs and mathematically impossible comparison
  outcomes; residual fractional, signed-rule-dependent and zero-divisor cases.
- Finite physical DECIMAL source values with explicit precision/scale checks,
  decimal arithmetic and exact integral casts; canonical ASCII string-source
  casts only under enforced binary comparison provenance.
- Exact finite TRY/SAFE_CAST failure NULLs and signed narrowing cast IS NULL /
  IS NOT NULL TRUE and NOT TRUE witnesses.
- Direct SQL/dbt, parser-capable dialect and executable DuckDB edge-case oracles.

**Verification is pending.** All five child acceptance criteria remain unchecked
until complete scope and final CI are independently verified. Further release
blockers include unsupported general decimal runtime laws, implicit/session
coercion, unbounded conversion rules and non-representable periodic inversion.
Do not close TASK-101 or unblock TASK-102 based on this increment alone.
