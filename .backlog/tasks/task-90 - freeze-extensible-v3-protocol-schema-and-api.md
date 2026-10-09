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
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
