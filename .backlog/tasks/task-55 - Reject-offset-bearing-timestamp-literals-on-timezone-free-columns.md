---
id: TASK-55
title: Reject offset-bearing timestamp literals on timezone-free columns
status: Done
updated_date: '2026-10-08'
assignee: []
created_date: '2026-10-08 11:35'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-46
  - TASK-53
  - docs/protocol.md
priority: high
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
docs/protocol.md (Typed predicate-domain literals) says contradictory timezone-free schema evidence combined with an offset-bearing literal produces Unknown, never an invented domain. The analyzer does not follow this, and claims exactness for a bound whose meaning depends on the engine.

Observed at 3a4d3c6 (duckdb dialect, schema `ntz TIMESTAMP WITHOUT TIME ZONE`):
- `WHERE ntz >= TIMESTAMP '2024-01-01 00:00:00+02'` is `exact` with no assumptions. The range bound keeps the literal text `2024-01-01 00:00:00+02`.
- DuckDB evaluates that literal as `2024-01-01 00:00:00` and drops the offset; PostgreSQL also ignores offsets in timestamp-without-time-zone literals; other engines may convert to the session time zone instead. A consumer reading the bound cannot know which value is meant without dialect knowledge it must not re-derive.

More generally, consumers must parse timestamp literal values, but the contract does not define a canonical value format for timestamps with and without time zone.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A comparison between a timezone-free timestamp column and a literal carrying a UTC offset yields Unknown with a documented residual reason, or a dialect-proven normalized value without offset text; it is never exact with the raw literal text
- [x] #2 Protocol docs define the canonical literal value format for timestamps without time zone and with time zone, including how offsets are represented
- [x] #3 Every emitted timestamp domain bound uses the canonical format matching the constrained column kind
- [x] #4 Tests cover offset-bearing and offset-free literals against timezone-free, timezone-aware, and unqualified timestamp columns
- [x] #5 The differential suite includes offset-bearing timestamp literals and confirms every exact claim with the engine
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Timestamp bounds now normalize into canonical wall-clock text with optional +HH:MM/-HH:MM
offsets. Explicit timezone-free schema evidence rejects offset-bearing literals as
Unknown with a literal_type_mismatch residual; malformed timestamp strings also remain
residual. Dialect-agnostic regression tests cover offset spellings, timezone-qualified
and unqualified schemas, and canonical bounds. DuckDB differential cases re-evaluate
exact bounds against physical rows.
<!-- SECTION:NOTES:END -->
