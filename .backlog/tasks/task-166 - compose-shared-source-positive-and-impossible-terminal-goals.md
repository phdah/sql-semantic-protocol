---
id: TASK-166
title: Compose shared-source positive and impossible terminal goals
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-03'
labels: []
milestone: m-3
dependencies:
  - TASK-165
references:
  - TASK-85
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-85 child 2/4.** Compose shared-source positive and impossible terminal goals. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-85 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `85-C166-P`, `85-C166-N`.
- **Finite variants:** two terminals one physical source; compatible and conflicting count/NULL goals.
- **Independent oracle/verification baseline:** source [1,1,2] => terminal A count3, terminal B WHERE key=1 count2; B requested 4 is Impossible with max 3.
- **Conditional/external evidence boundary:** Cannot multiply independent solutions or misclassify unknown shared inputs as Impossible. Owner TASK-166; release disposition pending review.
- **Original parent AC:** #1, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [x] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [x] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [x] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [x] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-85 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

Next: [TASK-167](task-167%20-%20compose-join-filter-group-and-another-window-set-terminal.md), only after completion and merge.

## TASK-166 implementation (2026-11-03)

Frozen before semantic implementation: [v1 task-owned complete-source/terminal inventory](../../docs/task-166-acceptance-freeze-v1.json). Original AC #1..#5 are accepted with the explicitly recorded owner dispositions below. TASK-85 remains open.

## Technical verification and formal signoff status (2026-11-03)

- TASK-166 implements and independently tests the bounded shared-source TRUE/NOT TRUE, duplicate, NULL, impossible and residual cases. The [v2 freeze](../../docs/task-166-acceptance-freeze.json) and [78-cell verification](../../docs/task-166-acceptance-verification.json) map the original child AC and preserve the immutable v1 inventory. Child AC #2, #3 and #4 have executable technical pass evidence. This checkmark denotes technical verification, not an independent identity review.
- The v1 pre-implementation freeze had an incorrect rejected/selected row partition for the impossible histogram and conflated a requested histogram with native output. Its original content remains in `docs/task-166-acceptance-freeze-v1.json`. Corrected v2 was reviewed via native differential oracles, but the historical freeze defect is not erased. The owner accepted this documented, versioned correction on 2026-11-03 and waived literal correctness of the historical prospective v1 fixture. The exception is retained in the audit; it is not presented as prospective correctness.
- Full Rust default/no-default, strict lint, documentation and dbt E2E passed 9/9 on reviewed implementation head `be2ab0b` ([run 41667049421](https://github.com/phdah/sql-semantic-protocol/actions/runs/41667049421)); pinned sql-tdg consumer qualification passed on an earlier implementation head with two documented preexisting timestamp exclusions. The owner explicitly accepted the technical Standards/Spec review and waived a distinct independent reviewer identity on 2026-11-03. This is owner signoff, **not** an independent GitHub approval. Final tracking-only head requires green CI before merge.
- Parent TASK-85 stays open and TASK-167 remains gated on TASK-166 merge.

## Owner acceptance and completion (2026-11-03)

- The owner explicitly accepted the TASK-166 review and approved completion and squash merge of PR #183.
- **AC #1 process deviation accepted:** the prospective v1 fixture contained incorrect SQL TRUE/NOT TRUE ordinals and conflated requested with native terminal B. V1 remains immutable, corrected v2 is independently exercised by the frozen acceptance tests. Owner accepts the documented historical exception rather than asserting that v1 was correct.
- **AC #5 reviewer-identity requirement waived by owner:** Matt Standards/Spec technical review from the PR author's connected GitHub account is accepted as technical evidence, not a distinct reviewer's approval. The owner authorized proceeding without a second GitHub reviewer.
- All five child ACs are signed off, subject to rechecking the final tracking-only CI before merge. TASK-85 remains open for the remaining children and independent parent AC signoff. TASK-167 is unblocked only after this Done child is merged into main.
