---
id: TASK-8
title: Emit deterministic protocol JSON and CLI
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-3
  - TASK-5
  - TASK-6
  - TASK-7
---

## Description

Provide the first usable protocol producer: accept SQL input, analyze it through the public library API, and emit only the versioned SQL Semantic Protocol JSON.

The emitted JSON is a public contract and must be deterministic for identical input and analysis configuration.

## Acceptance Criteria

- [x] Protocol domain values serialize to JSON that validates against the v0 JSON Schema.
- [x] Identical SQL, dialect, and analysis inputs produce byte-identical JSON output.
- [x] Serialized collection ordering follows the deterministic rules defined by the protocol contract.
- [x] The CLI accepts SQL from a practical non-hardcoded input such as stdin or a file and allows the caller to select the supported dialect.
- [x] Successful CLI output is protocol JSON and does not expose the raw sqlparser AST.
- [x] Fatal parse or input errors are reported distinctly from protocol diagnostics for unsupported semantics.
- [x] The CLI delegates parsing and analysis to the public library rather than duplicating logic.
