---
id: TASK-101
title: 'Prove typed arithmetic, overflow and conversion outcomes'
status: Done
assignee:
  - Philip Sjöberg
created_date: '2026-10-13'
updated_date: '2026-10-15 13:08'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-79
references:
  - TASK-70
  - TASK-88
  - TASK-90
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
- [x] #1 Normalize source-independent unary `+`/`-`, typed binary `+`/`-`/`*`/`/`/`%` and deterministic literal coercion with explicit type-width, precision, SQL NULL, division-by-zero and overflow/underflow guards. Handle the soundly provable integer/decimal families and emit typed residual for every unsupported operator/type pair.
- [x] #2 Produce strongest safely provable output domains (bounds, inclusivity, finite sets and excluded values) and reversible source constraints; certify inversion only when total/injective over the attested source domain, including signed boundaries and noninvertible or multiply-valued operations.
- [x] #3 Handle CAST, TRY_CAST and SAFE_CAST where parsing, type evidence and failure behavior are certified; cover identity/widening/narrowing and exact decimal or text conversions only under demonstrated semantic laws. Distinguish exception, NULL-on-failure and implicit/session-dependent coercions without silently claiming exactness.
- [x] #4 Prove TRUE and NOT TRUE (FALSE or UNKNOWN) outcomes, impossibility, and residual status for mixed arithmetic/comparison cases; do not infer global absence from finite sampling or unchecked arithmetic.
- [x] #5 Add direct SQL and dbt/catalog parity, all parser-capable dialect equivalence tests, DuckDB executable arithmetic/NULL/overflow/zero divisors and positive/negative/impossible/residual oracles. Update canonical Rust API, schema, semantic docs and capability evidence for each claimed family.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add focused public-seam regressions in `tests/task_101_arithmetic.rs` for correlated same-source output domains, signed underflow versus explicit widening, both TRUE and NOT TRUE residual directions, representative direct SQL/dbt parity, parser-capable dialect boundaries, and DuckDB overflow/zero-divisor behavior. Verify each new test fails for the intended semantic gap before implementation.
2. Make the smallest canonical-owner changes in `src/analysis/arithmetic_domains.rs` and related arithmetic/cast normalization needed to preserve correlation and fail closed where width, precision, coercion, exception, or runtime laws are not attested. Keep unsupported families explicit through existing canonical unknown/residual representations rather than adding consumer-specific behavior.
3. Reconcile semantic docs and capability evidence with the exact bounded families proved by tests, including the remaining residual boundaries; update schema/API docs only where the canonical contract changes.
4. Run focused integration tests and `cargo check` during each red-green slice, then run `cargo fmt`, strict all-target/all-feature clippy, `cargo test`, and `cargo doc --no-deps` for the public documentation changes.
5. Run the required two-axis `/code-review`, fix all in-scope findings, re-run affected verification, finalize every TASK-101 acceptance criterion with objective evidence, and commit the complete task-related diff to the current branch.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-10-15 audit: dependencies TASK-67 and TASK-79 are Done; the dedicated branch is clean and the existing 34-test TASK-101 suite passes. Completion remains blocked by an unformatted HEAD and uncovered soundness/evidence gaps, notably same-source Cartesian expansion, signed underflow coverage, both witness directions for residual failures, and broader parity/oracle evidence. The implementation plan records focused closure without taking over TASK-88's global dialect-law certification.

2026-10-15 closure pass resolved the two-axis review findings: unary negation now requires attested width (red test explicit_integer_negation_minimum_is_unknown), ordinary CAST failure is canonically runtime_dependent (SignedIntegerCastFailureBehavior enum, emitter as_str, schema enum plus allOf, protocol/semantics docs), signed widths share one SignedIntegerWidth owner, panic-capable indexing was removed from the arithmetic and decimal paths, and multi-behavior DuckDB/output-domain tests were split into one-behavior tests.

Typed residuals now carry datatype evidence instead of only structural classes: arithmetic_operand_kind returns an owned label including the explicit signed-cast target width, and boolean-witness residuals qualify source columns with catalog-attested datatypes through a new column_datatype evidence closure supplied by analysis/queries.rs (metadata) and physical_realization/filters.rs (source schema parity). Output-domain residuals without a catalog context keep the structural label, which the doc comment states explicitly rather than over-claiming.

Verification for this pass: cargo fmt --all -- --check, cargo clippy --all-targets --all-features -- -D warnings, full cargo test (every suite 0 failed), cargo doc --no-deps, jq empty docs/coverage-manifest.json, and git diff --check all pass. tests/fixtures/protocol-signed-cast.json locks the full emitted signed_integer_cast contract (cast_kind, on_failure runtime_dependent, target_bits) alongside the explicit node assertion in tests/emission_contract.rs.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
## TASK-101: typed arithmetic, overflow and conversion outcomes

Canonical, source-independent typed arithmetic and conversion semantics for predicates and output domains, with strongest safely provable value domains and explicit residuals everywhere proof is unavailable.

**What changed**
- **Arithmetic and conversion proof.** Correlated same-source integer and decimal arithmetic evaluates each source value once (no Cartesian expansion); catalog-attested signed-width and precision/scale evidence, overflow and zero-divisor guards, modulo range proofs, and total plus non-total inversion rules are covered by tests. Ordinary `CAST` failure is canonically `runtime_dependent`; `TRY_CAST`/`SAFE_CAST` are `null` on failure. Unary negation requires an attested width, so the minimum representable value is no longer claimed exact.
- **One width owner.** New `SignedIntegerWidth` enum centralizes bit widths, limits, and fit checks; four duplicated width helpers were removed across the arithmetic, cast, output and boolean-witness paths.
- **Typed residuals.** `arithmetic_operand_kind` now emits an owned label carrying the explicit signed-cast target width, and boolean-witness residuals qualify source columns with catalog-attested datatypes via a new `column_datatype` evidence closure threaded from `analysis/queries.rs` metadata and `physical_realization/filters.rs` source schemas. Residuals without a catalog keep the structural label, documented as such rather than over-claimed.
- **Contract and docs.** `signed_integer_cast` now requires `cast_kind` and `on_failure`; updated `docs/protocol.md`, `docs/semantics.md`, `docs/coverage.md`, `docs/coverage-manifest.json`, and the active JSON schema (enum plus `allOf` conditions).
- **Hygiene.** Removed panic-capable indexing in the arithmetic and decimal-literal paths, dropped an unreachable match arm, and split multi-behavior tests into focused one-behavior tests.

**Tests**
- Direct SQL, dbt/catalog, and physical-plan adapter parity for the same canonical outcomes.
- Parser-capable dialect equivalence for shared arithmetic, casts and decimal literals.
- DuckDB executable oracles for overflow, zero divisors, narrowing casts, NULL three-valued logic, decimal and remainder outcomes.
- Full-document contract fixture `tests/fixtures/protocol-signed-cast.json` plus explicit node assertion in `tests/emission_contract.rs`.
- New residual evidence test `typed_residual_reasons_attest_datatypes_and_cast_widths`.

**Verification**
`cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, full `cargo test` (all suites pass), `cargo doc --no-deps`, `jq empty docs/coverage-manifest.json`, `git diff --check`.

**Risks / follow-ups**
- Global dialect-law certification remains with TASK-88; the `exception` cast-failure value is retained in the schema as vocabulary for future dialect attestation but is currently unreachable by any cast rule.
- Protocol 3.0.0 release stays gated by TASK-91; the required `cast_kind`/`on_failure` additions are a breaking emitted-contract change and must be released with a major bump.
<!-- SECTION:FINAL_SUMMARY:END -->

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
