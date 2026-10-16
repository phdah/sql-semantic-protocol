---
id: TASK-105
title: Classify temporal, JSON and vendor expression outcomes
status: Done
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-16'
labels: []
milestone: m-3
dependencies:
  - TASK-104
references:
  - 'TASK-70'
  - 'TASK-88'
  - 'TASK-92'
  - 'TASK-79'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Audit date/time, JSON/nested values and vendor functions against the existing schema-backed type evidence. Implement only independently certified deterministic semantics and retain auditable residuals for unsafe session-dependent behavior.

**TASK-70 child, step 5/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Inventory DATE/TIME/TIMESTAMP and interval expressions, casts/comparisons, calendar boundaries, time zones, DST and engine/session-dependent coercion laws by parser AST and dialect; certify exact typed result domains only where necessary execution laws and metadata are explicit.
- [x] #2 Classify JSON path extraction, array/struct/nested comparisons, JSON SQL NULL versus JSON null and vendor function/operator families. Implement deterministic, provable typed subsets and retain explicit residuals for missing type/path/engine evidence.
- [x] #3 Preserve extensibility for UDFs and opaque/vendor functions through typed residual or externally validated evidence; never silently evaluate a UDF or mark a stochastic/session-sensitive result exact.
- [x] #4 For certified variants provide reversible constraints where possible, typed output domains, canonical TRUE/NOT TRUE/impossible witnesses and comparison assumptions; ensure all other inventoried variants remain fail-closed with traceable reasons.
- [x] #5 Add direct SQL/dbt adapter parity and parser boundary tests for all relevant dialects, focused DuckDB differential oracles for executable semantics, residual regression tests, and machine-readable per-family coverage plus docs/schema changes.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Audit existing portable typed DATE/TIME comparisons, explicit timestamp-zone evidence, source-level NULL tests, and opaque temporal/JSON/vendor AST boundaries. Certify only deterministic literal identity casts whose explicit input and target types agree; preserve calendar validity and fail closed on precision, timezone, path, engine/session laws and opaque functions. Exercise public SQL analysis/domain/exactness, JSON emission and dbt artifacts with per-dialect parser boundaries and independent DuckDB oracles. Preserve TASK-70/TASK-91 release gates.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Public TDD seams confirmed: SQL analysis (domains and truth/exactness), JSON emission (full canonical document), dbt artifacts/schema analysis; independent DuckDB execution oracles.
- Certified subset: Gregorian DATE identity casts and literal YEAR/MONTH/DAY extraction; ordered DATE interval intersection, scalar negation and impossible bounds; valid calendar/clock representations; existing typed timestamp comparison assumptions; datatype-independent source SQL NULL witnesses.
- Added a 56-expression executable AST/dialect inventory plus three positional-alias fixtures under `docs/coverage-manifest.json.task_105_expression_evidence`; explicit parser boundaries prevent hash-comment and interval/array/struct syntax from becoming false semantic claims. Alias regressions run in every exposed dialect.
- Unknown outcomes preserve family-specific unsupported reasons and opaque function names. JSON document null/path extraction, nested equality, timezone/DST/interval execution, vendor/UDF/clock/stochastic functions remain fail-closed.
- Preserved literal domains through ungrouped single-local-source copies, with regression coverage for outer-join and ROLLUP NULL synthesis. Producer composition preserves DATE bounds.
- Adapter evidence: direct SQL, paired dbt catalog artifacts, manifest-declared schemas and feature-gated ODCS physical types use equivalent canonical outcomes; no new adapter-specific semantic rules or dependencies.
- Two-axis review: addressed the input-triggered Unicode suffix panic, positional local-column alias mismatch, precision-qualified literal overclaim, and loss of literal timezone qualification. Added checked UTF-8 suffix parsing and modifier-aware fail-closed normalization, preserving source comparison assumptions only for independently valid literals. Temporal precision and zone regression/oracle tests now use explicit AST evidence rather than unsafe timestamp spelling equivalence.
- Standards review: consolidated physical and analysis Gregorian validation under `src/temporal.rs`, preserving malformed-versus-impossible physical classifications; renamed the general expression residual label helper. Alias application has one shared analysis owner used by CTE/derived scopes.
- Final review closure: both reviewers verified all findings resolved. Literal precision is bounded to the certified 0..6 subset; all three positional-alias paths now have all-dialect fixtures and explicit parser-boundary expectations.
- Final local verification: full `cargo test --quiet --no-fail-fast`, `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo doc --no-deps` pass. Focused evidence includes 21 TASK-105 tests, 20 typed-domain tests, 21 differential tests and six physical type-validation unit tests. Two existing artifact-generated dbt E2E tests are explicitly ignored by the default suite and owned by the CI dbt job.
- Remote verification: all nine checks pass on PR #113, including default/no-default feature suites, lint/doc/dependency checks and dbt Core E2E: https://github.com/phdah/sql-semantic-protocol/actions/runs/38863120417 .
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Delivered TASK-105 in https://github.com/phdah/sql-semantic-protocol/pull/113 from `feat/task-105-temporal-json-vendor` to `main`. All five scoped acceptance criteria are verified. Certified DATE constants/identity casts/extraction, ordered DATE outcomes and datatype-independent source SQL NULL witnesses share canonical ownership and preserve domains through composition and positional aliases. Precision/zone/calendar/Unicode boundaries, JSON document null and opaque function execution fail closed with traceable reasons. The executable inventory covers 56 expressions and three alias cases over all exposed dialect names, with explicit parser limitations; SQL/dbt/ODCS parity and independent DuckDB oracles pass. Standards and spec review findings are resolved, all local gates pass, and all nine remote CI checks including dbt E2E pass. TASK-70 remains In Progress and TASK-91/protocol 3.0.0 remain gated; TASK-106 is the next queued child.
<!-- SECTION:FINAL_SUMMARY:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** This task certifies **the safe subset and explicit residual boundary**, not universal timezone/vendor/JSON or engine execution. TASK-88 owns the global dialect matrix; TASK-92 owns later optional opaque-evidence research.
- **Next:** After completion, pick **TASK-106**.
