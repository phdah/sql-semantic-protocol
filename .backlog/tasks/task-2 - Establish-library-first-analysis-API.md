---
id: TASK-2
title: Establish library-first analysis API
status: To Do
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-1
---

## Description

Establish the public library boundary for converting SQL text into the SQL Semantic Protocol. Parsing, semantic analysis, and protocol emission must be separate concerns, and the selected SQL dialect must be supplied by the caller rather than hardcoded.

The command-line binary should become a thin consumer of the same public library API that other applications can call directly.

## Acceptance Criteria

- [ ] `src/lib.rs` exposes the supported public entry point for converting SQL text and a caller-selected dialect into protocol domain values or a domain error.
- [ ] Parsing, semantic analysis, and protocol serialization are separate modules with explicit boundaries.
- [ ] sqlparser AST types do not appear in public protocol types.
- [ ] The SQL dialect is selected by the caller and is not hardcoded in analysis code.
- [ ] Library errors distinguish parsing failures from semantic-analysis failures.
- [ ] `main.rs` contains no semantic-analysis logic.
- [ ] Existing debug/probe behavior is removed from the production path or isolated from the public API.
