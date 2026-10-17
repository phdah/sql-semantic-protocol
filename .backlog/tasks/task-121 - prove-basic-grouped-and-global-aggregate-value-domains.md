---
id: TASK-121
title: Prove basic grouped and global aggregate value domains
status: To Do
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-71
references:
  - TASK-73
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-73 child 1/4.** Implement only prove basic grouped and global aggregate value domains in a focused PR against `main` after dependencies merge. The original TASK-73 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-73 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `73-C121-P`, `73-C121-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** COUNT(*), COUNT(expr), SUM, MIN, MAX, AVG; NULL/duplicate; empty global versus grouped.
- **Independent concrete oracle baseline:** Input [1,1,NULL] => COUNT(*)=3, COUNT(x)=2, SUM(x)=2, MIN/MAX=1, AVG=1; empty COUNT(*)=0.
- **Fail-closed boundary and owner:** Use typed decimal/overflow domain laws, not host-language guessed arithmetic. TASK-121 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-73 AC:** #1, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [ ] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [ ] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [ ] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-121; no claim of native engine execution without an executable oracle.
- [ ] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-73 parent open.
<!-- AC:END -->

## Next

Next: [TASK-122](task-122%20-%20prove-distinct-filter-having-and-multiple-group-identities.md), only after this child is Done and merged.
