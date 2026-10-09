---
id: TASK-91
title: Block 3.0.0 release until downstream generator acceptance
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-77
  - TASK-78
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
  - TASK-84
  - TASK-85
  - TASK-86
  - TASK-87
  - TASK-88
  - TASK-89
  - TASK-90
references: 
  - 'sql-tdg TASK-24..36'
  - 'PR #79'
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
One consolidated 3.0.0 release is requested. The existing Release Please PR #79 must remain unmerged until the generator-relevant protocol contract is complete and verified.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 All protocol TASK-66..90 and release-blocking coverage-manifest entries are Done; no unchecked acceptance criteria or undocumented exclusions remain.
- [ ] #2 Candidate Rust fmt/clippy/test/doc, dbt Core E2E, no-default-features, schema snapshots, dialect and cross-feature differential tests are green.
- [ ] #3 sql-tdg integration branch consumes pinned *protocol release-candidate Git SHA*, with no dependency on an already published 3.0.0 crate, and passes generator TASK-24..31, TASK-35 and TASK-36 acceptance where upstream-dependent.
- [ ] #4 Decide and document final scope for arbitrary UDFs, stochastic operators, nonterminating recursion and unavailable vendor engines with explicit fail-closed behavior. No universal-support claims.
- [ ] #5 Maintainer signs off complete feature/dialect matrix and final dbt Makefile workflow before PR #79 is made ready/merged. Only then publish v3.0.0 once and switch sql-tdg to the published crate.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
- [ ] #7 Verify CI for each supported dialect's shared and dialect-specific SQL equivalence to the **same canonical semantic outcomes** where meanings are equivalent, including safe conditional session laws, while DuckDB E2E executes the complete supported transformation and DML/DDL sequence.
- [ ] #8 Verify seeded randomized rejected predicate/column alternatives, multi-seed coverage of every feasible alternative, and exact per-terminal presence/absence against complete generated DuckDB output snapshots.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Coverage manifest and cross-repo gate (2026-10-09)

The executable coverage inventory lives in [docs/coverage-manifest.json](../../docs/coverage-manifest.json), with its readable [dialect matrix](../../docs/coverage.md) and [Rust fixtures](../../tests/coverage_manifest.rs). TASK-66 is Done as the reviewed inventory and routed gap list, **not as proof of all variants**; TASK-88 owns exhaustive parser/canonical dialect laws; TASK-89 owns cross-feature oracle and dbt fixture certification. Each sql-tdg TASK-24..31/35/36 maps to upstream TASK-67..90 in the matrix. sql-tdg TASK-43 must pin a protocol **Git commit SHA** before protocol 3.0.0 is published.

**Gate remains closed** for all cells marked unverified, operator-local, residual, or conditionally deferred behavior without a documented fail-closed proof. The maintainer has **approved** sql-tdg TASK-35 per-terminal rejection with seeded randomized rejecting alternatives, TASK-36's unified dbt `make all` plus required scripted DML/DDL gate, and conditional future deferral for unproved opaque behavior. Implementation, canonical equivalence across all supported dialect/variants, and full executable release evidence remain pending.

**Maintainer scope choices:** [docs/coverage-signoff.md](../../docs/coverage-signoff.md). They cover sql-tdg TASK-35's negative row meaning, TASK-36's DML test location, future-extensible opaque/unbounded deferrals and their fail-closed tests, and canonical versus native-engine certification. A scope approval is **not** final release approval.

## Approved decisions versus release approval (2026-10-09)

[The four maintainer scope decisions](../../docs/coverage-signoff.md) are approved, **not** final release approval. Protocol TASK-66..90, all variant/dialect equivalence cases, the seeded multi-terminal negative contract, physical-source constructive proofs, and sql-tdg TASK-31/35/36 whole-project dbt `make all` plus scripted DuckDB DML/DDL must finish before final TASK-91 sign-off or Release Please PR #79 merge. Safe future UDF/recursion/stochastic extensions tracked in TASK-92 are not permanently prohibited and are not v3 blockers unless explicitly advertised as supported.

## Inventory completion does not imply release acceptance

TASK-66 was closed as the 53-family, 251-variant, 13-dialect **inventory and ownership handoff** in protocol PR #88. Its status must not be used to infer that an unverified manifest cell has passed. TASK-67..90, sql-tdg TASK-24..31/35/36/43/44 and the executable final TASK-91 release gate are still required; do not merge Release Please PR #79 based on TASK-66 alone.
