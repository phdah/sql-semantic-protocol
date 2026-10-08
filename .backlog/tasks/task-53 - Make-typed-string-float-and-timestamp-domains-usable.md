---
id: TASK-53
title: 'Make typed string, float, and timestamp domains usable'
status: To Do
assignee: []
created_date: '2026-10-08 09:06'
updated_date: '2026-10-08 09:06'
labels: []
milestone: m-2
dependencies:
  - TASK-51
references:
  - TASK-46
  - sql-tdg TASK-21.2
  - sql-tdg TASK-21.5
  - sql-tdg tests/dbt_core_e2e.rs
  - sql-tdg tests/advanced_raw_sql.rs
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
TASK-46 made every typed string, floating-point, and timestamp predicate residual and replaced its domain with Unknown. The concerns behind that are real: collation, case sensitivity, CHAR padding, NaN and signed zero, and time zones. But the result is that, once schema evidence exists, consumers cannot generate data for the most common filters in real models. It is also inconsistent: the same string predicate is exact when no schema is supplied.

Observed at 2bc99b3, with typed schemas and the duckdb dialect:
- `name = 'x'` and `name IN ('x', 'y')` on VARCHAR give Unknown, residual. Without schema evidence `name = 'x'` is an exact include set.
- `f > 1.5` on DOUBLE gives Unknown, residual.
- `ts >= TIMESTAMP '2024-01-01 00:00:00'` on a TIMESTAMP (no time zone) column gives Unknown, residual. The canonical datatype collapses timestamp with and without time zone, so the protocol cannot tell them apart.
- The sql-tdg dbt end-to-end fixture (`status` string filter) and its window fixture (`category = 'keep'`) can no longer generate. Both worked against 1.0.

## Outcome
- Domains are always kept in their typed form. Unknown is reserved for conditions the protocol truly cannot describe.
- Exactness can be conditional on named comparison-semantics assumptions (for example binary string collation, no NaN values, a session time zone). These are listed deterministically on each scope and on composed semantics. Conditions that hold under every supported setting need no assumption.
- Callers may declare assumptions as facts about their warehouse; declared assumptions are recorded in the output and no longer listed as open.
- With no declarations the protocol never claims more than it can prove, and consumers can still tell which assumption stands between a domain and exactness.
- Timestamp time-zone awareness is preserved in schema evidence so that timestamp-without-time-zone comparisons can be exact.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Typed string, float, and timestamp predicates keep their include set, exclude set, or range domain instead of Unknown
- [ ] #2 Exactness can be conditional on named comparison-semantics assumptions, listed deterministically on scopes and composed semantics with the conditions that depend on each
- [ ] #3 Conditions whose result is the same under every supported collation, padding, NaN, and time-zone setting are exact without assumptions, and the docs state which forms qualify
- [ ] #4 The library, CLI, analysis manifest, and dbt path accept caller-declared comparison semantics; declared assumptions are recorded in the emitted bundle and are not listed as open
- [ ] #5 Analysis with and without schema evidence applies the same comparison-semantics rules to string literals, so the same predicate cannot be exact in one and residual in the other
- [ ] #6 Canonical schema evidence distinguishes timestamps with and without time zone from catalog, manifest-declared, ODCS, and caller-supplied types, without breaking 1.x consumers
- [ ] #7 Timestamp-without-time-zone comparisons with offset-free literals are exact; time-zone-aware comparisons are exact only when the literal carries an offset or a session time zone is declared
- [ ] #8 Floating-point ranges and sets state NaN and signed-zero membership and are exact under the documented assumption
- [ ] #9 Each observation in the description has a test for the undeclared case and the declared case, and the differential suite checks exact claims under declared assumptions
- [ ] #10 JSON Schema, protocol docs, README, and public API docs describe comparison-semantics assumptions and declarations
<!-- AC:END -->
