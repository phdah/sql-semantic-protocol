---
id: TASK-177
title: Freeze TASK-66 feature-by-13-dialect parser coverage
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-07'
labels: []
milestone: m-3
dependencies:
  - TASK-66
  - TASK-70
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-77
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
references:
  - TASK-88
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-88 child 1/4.** Freeze TASK-66 feature-by-13-dialect parser coverage. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-88 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `88-C177-P`, `88-C177-N`.
- **Finite variants:** 13 named dialects; every reviewed variant; parser accepted/unsupported statuses.
- **Independent oracle/verification baseline:** Full cross-product of approved stable TASK-66 variant IDs with 13 dialects; no missing rows or implicit skip.
- **Conditional/external evidence boundary:** Parser success alone never certifies outcome semantics or vendor engine execution. Owner TASK-177; release disposition pending review.
- **Original parent AC:** #1, #2, #4, #5, #7, #8.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-88 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

Next: [TASK-178](task-178%20-%20certify-canonical-equivalence-for-shared-dialect-ast-variants.md), only after completion and merge.

## Implementation evidence (2026-11-07, not final sign-off)

- `docs/task-177-acceptance-freeze.json` v1 freezes the entire TASK-66
  251-variant x 13-dialect inventory (3,263 named cells) and explicitly retains
  the 3,002 unverified, 105 representative-only and 156 fixture-tested parser
  statuses. Sparse overrides are authoritative; family-level fixture evidence
  is not inherited by other variants.
- `tests/task_177_dialect_inventory.rs` checks complete coverage, individual
  fixture provenance, no invented SQL, no implicit dialect skips and the
  conservative absence of canonical/native-execution claims. Named
  `88-C177-P` and `88-C177-N` oracles require complete physical source,
  selected, NOT TRUE and UNKNOWN bags and compare the same portable SQL
  through all 13 public analysis dialects.
- The new machine-readable registry is linked by
  `docs/coverage-manifest.json.task_177_dialect_parser_acceptance`.
- This is an **in-progress bounded evidence freeze**, not acceptance sign-off.
  Missing exact SQL shapes, typed domain/source/terminal outcomes for the
  remaining variant/dialect cells, direct SQL/dbt/ODCS parity, native
  vendor/session certification, full CI, independent review and pinned-consumer
  gates remain blocking. All original AC #1..#5 stay unchecked, TASK-88
  remains open, and TASK-178 stays dependency-blocked until this child is
  fully certified, Done and merged.

## Parser sample inventory extension (2026-11-07)

- `docs/task-177-parser-samples.json` adds 251 task-owned sample SQL
  statements, one per reviewed TASK-66 feature variant.
- `docs/task-177-parser-snapshot.json` records independent CI-observed
  classifications for the 3,263 example SQL/dialect pairs:
  **3,208 parsed, 55 parser-rejected**. The 55 sample rejections belong to
  12 named variant families. No silent dialect skip or vendor execution
  claim is permitted.
- `docs/task-177-parser-boundaries.json` names all 55 rejected
  sample/dialect pairs, with conditional ownership and *unapproved*
  release disposition. Alternative native syntax requires independent
  evidence, not automatic fallback or claimed impossibility.
- `docs/task-177-acceptance-audit.md` maps all five original AC
  individually and enumerates remaining mandatory semantic, typed physical,
  SQL/dbt/ODCS, native session, consumer and independent-review gates.
  **Do not mark the child Done merely because parser and CI tests pass.**
