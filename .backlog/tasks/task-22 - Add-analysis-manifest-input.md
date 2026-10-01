---
id: TASK-22
title: Add analysis manifest input
status: To Do
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

- [ ] A versioned manifest contract is documented and validated.
- [ ] The manifest can reference any number of SQL files and inline SQL strings.
- [ ] Every manifest input can specify its own dialect, defaulting through a documented bundle-level rule when omitted.
- [ ] Dialect names continue to resolve through sqlparser rather than a project-maintained whitelist.
- [ ] The manifest can specify final/all-layer output scope and zero or more explicit targets.
- [ ] Relative file paths resolve deterministically relative to the manifest location.
- [ ] Duplicate input identities and invalid configuration are rejected explicitly.
- [ ] The CLI can execute an analysis directly from a manifest file.
- [ ] Equivalent manifest-driven and direct CLI/API invocations produce equivalent deterministic protocol semantics.
- [ ] Tests cover mixed dialects, mixed inline/file inputs, targets, and relative paths.
