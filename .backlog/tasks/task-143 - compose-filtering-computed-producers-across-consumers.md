---
id: TASK-143
title: Compose filtering/computed producers across consumers
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-31'
labels: []
milestone: m-3
dependencies:
  - TASK-142
references:
  - TASK-78
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-78 child 3/4.** Implement only compose filtering/computed producers across consumers in a focused PR against `main` after dependencies merge. The original TASK-78 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-78 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `78-C143-P`, `78-C143-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** CTE WHERE; computed aliases; downstream JOIN/count/write; materialization.
- **Independent concrete oracle baseline:** raw [1,2,3] WHERE k>1 => stage [2,3]; two consumers share exactly same stage bag.
- **Fail-closed boundary and owner:** No intermediate source insertion; preserve producer ordering and row filters. TASK-143 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-78 AC:** #1, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-143; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-78 parent open.
<!-- AC:END -->

## Next

Next: [TASK-144](task-144%20-%20certify-raw-dbt-multistep-scopes-and-task-78-signoff.md), only after this child is Done and merged.

## Signoff (2026-10-31)

All five child acceptance criteria are checked with the following explicit disposition:

- **AC #1 (one-time maintainer exception):** The first task-owned acceptance matrix was committed after semantic changes had begun, so the literal preimplementation ordering requirement was not met. On 2026-10-31 the maintainer explicitly accepted a one-time exception for **timing only**. The complete v4 matrix remains in [the immutable task-owned freeze](../../docs/task-143-acceptance-freeze.json), with v1, v2 and v3 retained as historical evidence. This does not retroactively claim that a preimplementation freeze occurred or waive any mandatory fixture, proof or adapter obligation.
- **AC #2-#4:** Nine complete typed cases (three read Exact, two write Exact, one CHECK Impossible, two read Residual, one width-related write Residual) are accounted for in public canonical/physical tests and independent DuckDB source, producer and target oracles. Five SQL families across 14 exposed parser names yield 70 pinned parser cells; nine cases across three adapters yield 27 named dispositions. Native vendor behavior beyond DuckDB is not asserted.
- **AC #5:** A separate Matt `code-review` Standards/Spec pass inspected the implementation and complete finite matrix, and found no additional actionable code defects: [review](https://github.com/phdah/sql-semantic-protocol/pull/169#pullrequestreview-5723429198). The previous integer-width false-Exact bug has been repaired and independently regression-tested. The implementation-head [CI run](https://github.com/phdah/sql-semantic-protocol/actions/runs/41205685783) completed all nine jobs successfully on `35a37f9bc9b6eee1f0f6dc5bd7954c87b3442e0b`. **The final signoff commit must also pass CI before merge**, with its SHA/run recorded in PR #169. The GitHub review is COMMENTED through the owner's connection, not a separate-account approval.

**Child disposition:** TASK-143 is Done on this implementation branch, conditional on the final PR-head CI merge gate. Do not merge with failed or incomplete final checks. **Parent TASK-78 remains open and unsigned**, and TASK-144 remains blocked until this child is both Done and merged on `main`.
