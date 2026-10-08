---
id: TASK-56
title: Treat equivalent timezone-aware timestamp literal spellings consistently
status: To Do
assignee: []
created_date: '2026-10-08 11:35'
updated_date: '2026-10-08 11:35'
labels: []
milestone: m-2
dependencies:
  - TASK-55
references:
  - TASK-53
  - docs/protocol.md
priority: medium
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
The same timezone-aware literal gets different exactness depending on how it is spelled. Consumers receive inconsistent answers for equivalent SQL.

Observed at 3a4d3c6 (duckdb dialect, schema `tz TIMESTAMP WITH TIME ZONE`):
- `WHERE tz >= TIMESTAMPTZ '2024-01-01 00:00:00+00'` is `conditional` on `session_time_zone`.
- `WHERE tz >= TIMESTAMP WITH TIME ZONE '2024-01-01 00:00:00+00:00'` is `exact` with no assumptions.

Both literals carry an explicit UTC offset, which the contract says makes timezone-aware comparisons unconditionally exact.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every timezone-aware timestamp literal spelling sqlparser accepts (for example TIMESTAMPTZ, TIMESTAMP WITH TIME ZONE, dialect-specific forms) with an explicit offset in any accepted format (Z, +HH, +HH:MM, +HHMM) yields the same exactness and the same canonical bound value
- [ ] #2 Literals without an offset compared with timezone-aware columns remain conditional on session_time_zone regardless of spelling
- [ ] #3 The completeness suite covers each spelling and offset format for every dialect that parses it
- [ ] #4 Protocol docs list the accepted spellings and offset formats
<!-- AC:END -->
