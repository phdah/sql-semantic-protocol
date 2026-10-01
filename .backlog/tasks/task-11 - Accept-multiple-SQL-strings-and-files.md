---
id: TASK-11
title: Accept multiple SQL strings and files
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
dependencies:
  - TASK-10
---

## Description

Extend the library and CLI so one analysis invocation can consume an arbitrary number of SQL inputs. Inputs may be supplied as SQL strings, files, or a mixture of both.

The interface must have no hard-coded maximum input count. Each input must retain enough identity to report parse and analysis diagnostics against the correct source. Existing single-query usage must remain straightforward.

Dialect handling must continue to delegate to the sqlparser dialect registry rather than introducing a project-specific dialect whitelist.

## Acceptance Criteria

- [x] The public library API accepts a collection of SQL input units rather than requiring exactly one SQL string.
- [x] An input unit records its SQL text and stable source identity without leaking sqlparser AST types into the public API.
- [x] The CLI accepts repeated string inputs and repeated file inputs in the same invocation.
- [x] Input ordering is deterministic and documented.
- [x] There is no fixed maximum number of inputs in the API or CLI.
- [x] Parse, file, and analysis errors identify the input that caused them.
- [x] Every dialect supported through the existing sqlparser dialect resolver remains available.
- [x] The existing one-string, one-file, and stdin workflows continue to work or have a documented migration path.
- [x] Tests cover multiple strings, multiple files, and mixed string/file input.
