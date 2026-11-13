---
id: TASK-194
title: Certify set-operator bags, NULL equality, and column alignment
status: In Progress
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-13'
labels: []
milestone: m-3
dependencies:
  - TASK-193
references:
  - TASK-88
  - TASK-178
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-178 scope-split follow-up; TASK-88 release-blocking child.** **22 original variants:** `sets` (22), **286 dialect cells**. Include UNION/INTERSECT/EXCEPT DISTINCT/ALL, NULL tuple equality, branch order and shared physical bags, nested/positional/BY NAME/CORRESPONDING alignment, incompatible arity, and MINUS as the appropriate parser-dependent spelling. Original TASK-178 AC #1..#5 and TASK-88 AC #1..#8 remain binding at their mapped owner/final gate; completing this bounded task does not approve unrelated matrix rows, vendor runtime support or protocol 3.0.0. Source of truth: [scope split](../../docs/task-178-scope-split.json). Do not introduce parent-child cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded fixture IDs and evidence gates

- **fixture IDs:** `88-C194-P`, `88-C194-N` (positive and rejected/Impossible physical cases).
- **DuckDB / 13 dialect parser seam:** compare independent closed-source/terminal bags, complete emitted typed protocol outcomes and NULL/boundary evidence; never infer other vendor runtime execution.
- **Residual cases:** preserve explicit unapproved semantic gaps, required proof and downstream owner; never promote an unimplemented mandatory core variant to Exact or approved Residual.
- Freeze the task-owned matrix in `docs/coverage-manifest.json` before changing semantic code.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Freeze exactly 22 original SQL case IDs x 13 dialect statuses in a task-owned v1 fixture; pin full source tuples/occurrence counts, expected terminal and rejected multiset bags, column order/types and named parser/analyzer boundaries.
- [ ] #2 Verify tuple-level (not per-column) multiset minimum/subtraction/deduplication and NULL equality with independent DuckDB goldens, including shared-source branches, positional columns, nested sets and negative/impossible counterpart bags.
- [ ] #3 Require strongest sound canonical output domains, stable source dependency identities, full-document semantic equivalence for meaning-preserving accepted spellings and explicit fail-closed unsupported alignment/arity outcomes; route true parser-only boundaries to TASK-191 without calling an unsupported case certified.
- [ ] #4 Check representable direct SQL/dbt/ODCS equivalence for actual type/source facts, forward evidence to TASK-197/TASK-180, and pass final-head default/no-default Rust and dbt tests plus relevant pinned consumer verification.
- [ ] #5 Complete exact-head rustfmt, strict Clippy, default/no-default Rust tests and docs, dbt E2E, active schema/goldens and relevant pinned sql-tdg consumer checks. Independently review every bounded acceptance cell and record the linked evidence in `docs/coverage-manifest.json`; TASK-88 parent and TASK-91 release remain unapproved until TASK-180 and separate parent signoff.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to TASK-195. TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.

## Implementation evidence (PR #200)

- Task-owned v1 matrix: `docs/task-194-acceptance-freeze.json` (22 original SQL cases × 13 parser dialects).
- Native closed-source and terminal multiset regression: `tests/task_194_native_set_bags.rs` (20 native cases); `CORRESPONDING` parser rejection and incompatible arity remain explicit negative cases.
- Public full-document and source-identity checks: `tests/task_194_dialect_semantics.rs`.
- SQL/dbt/ODCS source-type parity and sound projected domain checks: `tests/task_194_adapter_parity.rs`.
- Final-head CI, independent semantic cell review, and pinned consumer compatibility are **not yet signed off**. Do not mark Done on the presence of tests alone.
