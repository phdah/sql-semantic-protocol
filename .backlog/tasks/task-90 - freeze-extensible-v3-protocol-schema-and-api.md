---
id: TASK-90
title: Stabilize the v3 contract with compatible extensions
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
  - TASK-185
  - TASK-186
  - TASK-187
  - TASK-188
references: 
  - 'docs/protocol.md'
  - 'schema/protocol.schema.json'
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Consumers need a versionable typed ABI rather than another major upgrade for every new operator.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Audit every public Rust model, JSON schema, serde/emission format and CLI against the complete canonical witness/state contracts before v3 release.
- [ ] #2 Define forward-compatible optional capability fields, version/capability negotiation or explicitly version-gated extensions, unknown-tag/unsupported handling and strict validation guarantees.
- [ ] #3 Classify changes as additive/nonbreaking versus incompatible, with golden v2-to-v3 fixtures and consumer migration guide; no hidden dialect-specific alternate AST in protocol.
- [ ] #4 Document exactness semantics and source identities as stable invariants; ensure minor releases can add representable variants without silently changing existing proofs.
- [ ] #5 Validate pinned sql-tdg pre-release consumer against final v3 release-candidate commit and schema, plus stable byte-for-byte deterministic serialization.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
- [ ] #7 Preserve a typed extensibility path for future function/opaque operation contracts from structured user declarations, vendor catalog introspection, dbt-supplied macro/function metadata or schema adapters, without treating those unproven future semantics as v3-supported.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#7 remain required**. Interpret and verify them through the frozen TASK-90 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: public Rust/JSON/CLI contract inventory, stable typed identities/proof strength, deterministic serializer, full schema validation and golden v2 -> v3 changes.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: pinned sql-tdg pre-release consumer validates canonical source/witness/write examples against final v3 candidate.
3. **Conditional and non-exact boundary:** Extensibility: unknown tags and future typed UDF/operation metadata have defined version gates and fail-closed behavior without premature v3 claims.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#7 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Maintainer scope (2026-10-09)

Approved deferred-extensibility rule in [docs/coverage-signoff.md](../../docs/coverage-signoff.md): non-modeled UDFs, randomness, recursion and vendor session assumptions fail closed **now**, but may be supported in future versions by a typed, independently validated source-independent evidence contract. TASK-92 tracks post-v3 exploration and is **not** a blocker of the consolidated 3.0.0 release.

## Bounded dependency-ordered child execution queue (2026-10-17)

TASK-90 is an **independent parent signoff** and cannot be closed from a narrow passing CI subset. All original AC remain mandatory and unchecked until every child below is Done and merged. Choose first unfinished child with satisfied upstream dependencies; no child depends on TASK-90. `docs/backlog-child-queue-plan.json` is the versioned planning inventory, and each child must register a frozen, executable task-specific acceptance matrix before semantics change.

- **[TASK-185](task-185%20-%20inventory-rust-json-cli-witness-and-write-public-contract.md)** (1/4): public model identities; emitted schema; deterministic bytes; all supported modes. Fixtures `90-C185-P`, `90-C185-N`.
- **[TASK-186](task-186%20-%20define-typed-capability-extensions-and-fail-closed-unknown-tags.md)** (2/4): future function/opaque metadata; optional fields; version gating; strict unknown-tag handling. Fixtures `90-C186-P`, `90-C186-N`.
- **[TASK-187](task-187%20-%20certify-v2-to-v3-migration-and-canonical-compatibility.md)** (3/4): breaking versus additive; v2/v3 goldens; migration docs and schema snapshots. Fixtures `90-C187-P`, `90-C187-N`.
- **[TASK-188](task-188%20-%20pin-sql-tdg-pre-release-consumer-and-certify-final-task-90-gate.md)** (4/4): final candidate SHA; consumer reads source/row/write cases; serialization CI. Fixtures `90-C188-P`, `90-C188-N`.

**Original AC ownership:** #1 -> TASK-185, TASK-187, TASK-188; #2 -> TASK-186, TASK-188; #3 -> TASK-187, TASK-188; #4 -> TASK-185, TASK-186, TASK-187, TASK-188; #5 -> TASK-188; #6 -> TASK-185, TASK-186, TASK-187, TASK-188; #7 -> TASK-186, TASK-188.

**No premature Done:** mandatory typed exactness cannot be deferred as residual; unverified cells block completion. Positive/NOT TRUE rows, nullable interval output domains, full DuckDB bags, adapter/13-dialect evidence, reviewed conditional vendor cases and final CI are necessary. TASK-91 remains a distinct release decision after TASK-90 closure.
