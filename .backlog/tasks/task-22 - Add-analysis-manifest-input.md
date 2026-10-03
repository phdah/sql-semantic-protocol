---
id: TASK-22
title: Add analysis manifest input
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-11
  - TASK-21
---

## Description

Add a declarative manifest format for larger analysis bundles so callers do not need to encode every input and option as command-line arguments.

The manifest should describe SQL strings/files, stable source identities, dialect selection, requested targets, and output scope. It should also allow per-input dialects so a single protocol bundle can combine transformations originating from different SQL dialects when their relation identities can be resolved safely.

Keep the manifest independent from sqlparser implementation types and avoid introducing a dependency unless the existing dependency set cannot represent the chosen format cleanly.

## Acceptance Criteria

- [x] A versioned manifest contract is documented and validated.
- [x] The manifest can reference any number of SQL files and inline SQL strings.
- [x] Every manifest input can specify its own dialect, defaulting through a documented bundle-level rule when omitted.
- [x] Dialect names continue to resolve through sqlparser rather than a project-maintained whitelist.
- [x] The manifest can specify complete-bundle or explicit-target output scope, consistent with TASK-15, and zero or more explicit targets.
- [x] Relative file paths resolve deterministically relative to the manifest location.
- [x] Duplicate input identities and invalid configuration are rejected explicitly.
- [x] The CLI can execute an analysis directly from a manifest file.
- [x] Equivalent manifest-driven and direct CLI/API invocations produce equivalent deterministic protocol semantics.
- [x] Tests cover mixed dialects, mixed inline/file inputs, targets, and relative paths.
