---
id: TASK-46
title: Type domain literals against source column datatypes
status: In Progress
assignee: []
created_date: '2026-10-07 18:11'
updated_date: '2026-10-08'
labels: []
milestone: m-2
dependencies:
  - TASK-43
  - TASK-47
references:
  - TASK-42
  - docs/protocol.md
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
A domain is only exact if its bounds mean the same thing to the protocol and to the warehouse. Today literal bounds keep their lexical type, whatever the column type.

Observed at 2e6998e:
- `WHERE d > '2024-01-01'` on a DATE column emits a range with a string literal bound.
- `WHERE a = 1.5` on an INTEGER column emits include {1.5}; SQL comparison semantics make it unsatisfiable or dialect-dependent.
- Timestamp-with-time-zone columns are normalized to `timestamp`, but a literal with an offset is not defined relative to that normalization.
- String range bounds (`name > 'b'`) depend on collation and case sensitivity. Trailing-space handling for CHAR, float NaN and -0, and decimal scale are not defined by the contract.

## Outcome
When typed schema evidence exists for a constrained column, every literal in its domain is either converted to the column's canonical datatype following the dialect's comparison semantics, or the condition is residual or Unknown with a reason. The contract states the comparison semantics each datatype family assumes, and comparisons whose meaning depends on warehouse settings the protocol cannot see are never marked exact.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 With schema evidence, domain literals are expressed in the constrained column canonical datatype, or the condition is residual or Unknown with a reason
- [ ] #2 Lossy or dialect-dependent coercions (string to date or timestamp, decimal to integer, out-of-range values, offset timestamps vs normalized timestamps) are never presented as exact
- [ ] #3 Protocol docs state the equality and ordering semantics assumed per datatype family, including string collation and case sensitivity, CHAR padding, float NaN and signed zero, decimal scale, and timestamp time zone handling
- [ ] #4 String range comparisons and other comparisons whose result depends on unknown warehouse settings are residual unless the contract defines them exactly
- [ ] #5 Without schema evidence, behavior is documented and never claims more than the lexical literal supports
- [ ] #6 Tests cover every datatype family with exact, coerced, and residual cases
<!-- AC:END -->
