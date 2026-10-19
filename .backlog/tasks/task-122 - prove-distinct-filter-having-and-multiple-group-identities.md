---
id: TASK-122
title: Prove DISTINCT FILTER HAVING and multiple group identities
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-19'
labels: []
milestone: m-3
dependencies:
  - TASK-121
references:
  - TASK-73
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-73 child 2/4.** Implement only prove distinct filter having and multiple group identities in a focused PR against `main` after dependencies merge. The original TASK-73 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-73 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `73-C122-P`, `73-C122-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** COUNT DISTINCT; FILTER WHERE; mixed HAVING predicates; two aggregates and aliases.
- **Independent concrete oracle baseline:** Group k=1 with values [1,1,NULL] => COUNT DISTINCT=1 and COUNT(x)=2; HAVING COUNT(x)>1 survives.
- **Fail-closed boundary and owner:** Mixed FALSE/UNKNOWN rejected groups must be proven against entire group bag. TASK-122 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-73 AC:** #1, #2, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-122; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-73 parent open.
<!-- AC:END -->

## Next

Next: [TASK-123](task-123%20-%20compose-aggregates-after-filtered-joins-and-classify-grouping-variants.md), only after this child is Done and merged.

## Final Summary

<!-- SECTION:SUMMARY:BEGIN -->
Verified all four frozen TASK-122 cases, full source and terminal DuckDB bags,
group-level TRUE/FALSE/UNKNOWN exclusions, exact physical contributor identities,
13 exposed parser-dialect paths and canonical SQL/dbt/ODCS parity in
[the acceptance audit](../../docs/task-122-acceptance-audit.md).
The semantic head `62642b51a763f8fcc0e7db63df13768a8364e545` passed all nine GitHub CI jobs
([run 39275972342](https://github.com/phdah/sql-semantic-protocol/actions/runs/39275972342)).
Final metadata-head CI must also pass before PR #132 merges. TASK-73 stays open;
TASK-123 is blocked until this child PR merges.
<!-- SECTION:SUMMARY:END -->
