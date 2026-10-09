---
id: TASK-92
title: Evaluate typed external evidence for opaque and runtime SQL semantics
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels:
  - future
  - semantics
milestone: null
dependencies: []
references:
  - 'TASK-66'
  - 'TASK-78'
  - 'TASK-79'
  - 'TASK-87'
  - 'TASK-88'
  - 'TASK-90'
  - 'docs/coverage-signoff.md'
priority: medium
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A **post-v3 future feature**, not a 3.0.0 release blocker: support safely provable semantics for previously opaque SQL operations when callers can supply typed, verifiable evidence. The maintainer approved treating arbitrary UDFs, unknown functions, environment-dependent operators, stochastic behavior, recursion and vendor-specific session laws as **conditionally deferred**, not permanently unsupported. Existing parsing and unsupported/residual handling must continue to fail closed unless exact proof is available. This task must not cause a premature claim of supported SQL during v3.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Design parser-independent, versioned typed function/operation contracts: argument/result value domains, null handling, determinism, side effects, cardinality, source dependencies, evaluation order and error/overflow behavior.
- [ ] #2 Investigate evidence adapters for user-supplied structured declarations, database catalog/introspection SQL, dbt metadata/macros/tests, data contracts (including ODCS) and declared vendor/session settings; only admit verified facts with recorded provenance.
- [ ] #3 Define a trust boundary and capability validation so untrusted/incomplete evidence cannot silently upgrade residual semantics to exact; distinguish declared assumptions, tested laws, and generated witness proofs.
- [ ] #4 Evaluate seeded stochastic distributions, provably terminating/bounded recursion, conditional coercion/collation/timezone rules and interpreted UDF subsets without asserting arbitrary function or remote side-effect equivalence.
- [ ] #5 Provide unit, cross-dialect, adapter-parity and DuckDB differential fixtures for supported extensible cases, including negative and unverifiable evidence that must fail closed.
- [ ] #6 Publish a backward-compatible extension and consumer migration plan; keep v3 coverage-manifest and release-blocking acceptance truthful.
<!-- AC:END -->

## Delivery guidance

Target a version after the consolidated 3.0.0 release unless the maintainer separately reprioritizes it. Do not import SQL parser logic into sql-tdg or interpret UDFs in the consumer. Source-specific discovery ends at an adapter boundary; all proofs and outcome obligations must use one canonical typed contract.
