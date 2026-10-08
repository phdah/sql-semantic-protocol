---
id: TASK-54
title: Report accurate residual reasons and normalize boolean predicates
status: To Do
assignee: []
created_date: '2026-10-08 09:06'
updated_date: '2026-10-08 09:06'
labels: []
milestone: m-2
dependencies:
  - TASK-53
references:
  - TASK-43
  - TASK-46
  - TASK-48
priority: medium
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
Consumers turn residual conditions into user-facing errors, so a wrong reason, clause, or duplicate entry misleads users and hides the real blocker.

Observed at 2bc99b3:
- Typed string, float, timestamp, lossy-coercion, and out-of-range literal residuals all use reason `computed_expression`, although no expression is computed.
- `WHERE ghost = 1` with an unknown column yields `analysis_diagnostic` residuals with clauses `row_set_operator` and `where`; the condition is in WHERE only.
- `WHERE flag` emits two identical `computed_expression` residuals, and the daily-revenue chain emits four identical `column_comparison` residuals.
- `WHERE flag` on a BOOLEAN column is residual while the equivalent `WHERE flag = true` is exact; `WHERE NOT flag` likewise.

## Outcome
Each residual names its true cause through a documented stable reason code and the clause where the condition appears. A condition is never listed twice, and equivalent boolean forms have the same exactness.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every residual reason code is documented with its cause, and typed-literal residuals (comparison semantics, literal type mismatch, lossy coercion, out-of-range literal, unknown schema column) each have a distinct code
- [ ] #2 Residual clauses always name the clause containing the condition
- [ ] #3 Residual lists contain no duplicate entries; distinct conditions with the same reason have distinct identities
- [ ] #4 Bare boolean column predicates, their negation, and IS TRUE / IS FALSE / IS NOT TRUE / IS NOT FALSE forms on boolean columns are exact with the same domains as the equivalent comparison, including NULL membership
- [ ] #5 Tests assert reason code, clause, and identity for every residual path in the analyzer
<!-- AC:END -->
