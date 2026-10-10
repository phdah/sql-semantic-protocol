---
id: TASK-94
title: Realize multi-parent joins and repeated-source DAGs
status: Done
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
- [x] #1 Compose canonical equi, outer, semi, anti, cross and supported many-to-many join evidence through independent multi-layer parent producers where local operators carry sufficient proof.
- [x] #2 Reconcile self-joins, aliases, repeated producers and overlapping physical sources with shared row identities; model multiplicity and NULL extensions without falsely independent key assignments.
- [x] #3 Construct compatible nonzero and zero terminal counts, joining and rejected-row witnesses with complete physical source assignments and necessary absence/no-partner evidence.
- [x] #4 Conserve exact lineage, key types, value domains, cardinality laws and materialization/write-kind boundaries; report unsupported join topology or conflicting constraints explicitly.
- [x] #5 Add unit, all applicable dialect parser-boundary checks and DuckDB E2E tests for joined materialized parents, unmatched rows, NULL keys, duplicates, impossible/residual shared-source cases.
<!-- AC:END -->

## Implementation and acceptance evidence (PR #94)

- **#1 Join kinds and bag multiplicity.** A canonical `JoinPopulationPattern`
  encodes complete physical key populations: synchronized distinct non-NULL
  integer keys for one-to-one joins; common non-NULL keys for bag
  multiplicities (bounded integer factors); and entirely empty physical
  sides for unmatched, outer NULL-extension and SEMI/ANTI absence.
  `JoinPopulationPattern::output_rows` enforces the typed local join law
  before any constructive witness is emitted. INNER, LEFT, RIGHT, FULL,
  LEFT/RIGHT SEMI and LEFT/RIGHT ANTI are supported where local
  `JoinWitnessDirection::Exact` evidence exists. CROSS, unknown join
  variants and multi-operator joins without exact local evidence remain
  typed residual, not speculative proofs; local variant ownership stays
  with TASK-71.
- **#2 Shared row identities.** `physical_join_population_count_plan` traces
  each join key through complete, plain-copy, datatype-certified producer
  DAG edges. It uses exactly one closed-world assignment for each independent
  physical relation, including when two aliases or named producers refer
  to the same physical source. Distinct instance identities survive in
  typed source-bound row variables. Self-join counts cannot assume
  contradictory left/right source populations.
- **#3 Positive, negative and coupled goals.** Complete `Rows`,
  `ClosedWorld(EntireRelation)`, `JoinPopulation`, `JoinPair` or
  `NoMatchingPartner`, and `OutputRows` obligations jointly certify the
  supported exact cardinalities. Multiple output targets reconcile source
  counts and per-column key assignments before composing cases; conflicts
  among *sufficient* examples remain residual. A necessary transparent
  zero-row source and a positive INNER/SEMI output are impossible.
  Contradictory exact counts on one terminal are impossible regardless
  of alternative witnesses.
- **#4 Source-independent protocol contract.** The typed join population
  includes origin instance, physical source key, SQL join kind, pattern,
  source counts and exact result count. The active JSON schema, canonical
  serializer, Rust exports, protocol and semantics docs, and coverage
  inventory are updated. Unresolved source schemas, unsupported types,
  casts, unknown cardinalities, partial writes and unsupported joins fail
  closed. Existing typed lineage and strongest justified column outcome
  domains are retained unchanged.
- **#5 Verification.** Cross-dialect source and alias tests,
  multi-layer materialized parents, shared/self joins, bounded
  many-to-many products, NULL and duplicate bags, outer unmatched and
  no-partner cases, SEMI/ANTI parser-boundary checks, deliberate
  zero and impossible goals, schema emission checks, source provenance
  adapter parity, and DuckDB differential oracles are in the test suite.
  Full CI runs Rust tests, rustfmt, Clippy, docs, no-default-features
  and dbt Core E2E on this PR.

These complete the generic multi-parent join population child for
currently supported exact local join evidence. Tasks TASK-95..100 continue
to own higher-order group/window/set composition, projection inversion,
general negative cardinalities, ordered stateful DML, generator fixtures
and final TASK-68 sign-off. Neither TASK-68 nor the protocol 3.0.0
release gate is closed by TASK-94 alone.

## Implementation and verification

- **Parent:** [TASK-68](task-68%20-%20compose-witnesses-across-full-transformation-graphs.md); parent remains In Progress until all eight children are Done.
- **Delivery:** A focused PR into `main` with code, tests, API/schema/docs/adapter updates proportional to changed semantics, and passing CI.
- **Next:** After this task is Done, proceed to TASK-95; do not skip ahead.
- **Scope boundary:** Other TASK-70..91 work still owns individual missing operator variants, exhaustive conformance, the downstream generator, and protocol 3.0.0 release sign-off.

