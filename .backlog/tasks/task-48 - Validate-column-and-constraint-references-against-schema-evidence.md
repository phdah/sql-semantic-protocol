---
id: TASK-48
title: Validate column and constraint references against schema evidence
status: To Do
assignee: []
created_date: '2026-10-07 18:11'
labels: []
milestone: m-2
dependencies: []
references:
  - TASK-34
  - TASK-40
  - TASK-42
  - sql-tdg TASK-21.2
  - sql-tdg TASK-22
priority: medium
type: bug
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Why
Schema evidence (catalog, manifest-declared, ODCS, caller-supplied) exists so consumers can trust column references and types. References that do not match it currently pass silently.

Observed at 2e6998e:
- With manifest-declared fallback schemas, a model that reads a column missing from the declared columns analyzes successfully. Only an `unresolved_output_lineage` diagnostic is emitted outside composed semantics, and sql-tdg generated a source table without the column.
- Constraints whose columns are not in the relation schema are emitted as-is: `not_null` on `ghost`, `relationships` with `field: nonexistent_col`, and `unique` with `column_name: "lower(id)"` becoming `UniqueKey(["lower(id)"])`.
- Accepted values are not checked against the column datatype: string values on a numeric column produce no diagnostic.
- A `relationships` test whose `to` names a relation different from its `depends_on` target silently uses the dependency.

## Outcome
Whenever schema evidence exists for a relation, every column the analysis or a constraint references is validated against it. Mismatches produce blocking diagnostics on composed semantics or on the constraint set, never silent pass-through.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A query column reference absent from available schema evidence for its physical relation makes the composed semantics unresolved or carries a blocking diagnostic, for every schema source kind
- [ ] #2 Constraints naming a column absent from the relation schema, or naming an expression rather than a column, produce a constraint diagnostic and are not emitted as valid constraints
- [ ] #3 Accepted values that cannot be represented in the column datatype produce a constraint diagnostic
- [ ] #4 A relationships test whose to target disagrees with its dependency target fails or produces a diagnostic
- [ ] #5 Tests cover each case for catalog, manifest-declared, ODCS, and caller-supplied schemas
- [ ] #6 Protocol docs describe reference validation
<!-- AC:END -->
