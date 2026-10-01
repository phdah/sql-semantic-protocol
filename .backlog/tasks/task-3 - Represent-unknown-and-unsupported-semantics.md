---
id: TASK-3
title: Represent unknown and unsupported semantics
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-1
  - TASK-2
---

## Description

Make uncertainty a first-class part of semantic analysis. Any construct that parses successfully but cannot yet be analyzed must remain visible in the protocol as unknown or unsupported rather than being silently ignored or converted into an unjustified precise result.

Fatal syntax or parsing errors remain errors. Unsupported semantic analysis should preserve a partial protocol whenever doing so is safe.

## Acceptance Criteria

- [x] The protocol distinguishes fatal parse failures from successfully parsed but unsupported or unresolved semantics.
- [x] Analysis fallbacks produce explicit diagnostics or explicit unknown values instead of silent no-ops.
- [x] Diagnostics identify the affected semantic area closely enough for a consumer to understand what is incomplete.
- [x] Partial protocol output is preserved when unsupported constructs do not make the known semantics unsafe.
- [x] Tests cover representative unsupported expressions, statements, table factors, and functions.
- [x] Adding an unsupported sqlparser AST variant cannot silently remove semantic information.
