---
id: TASK-192
title: Certify predicate and structural-type outcomes across 13 dialects
status: In Progress
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-10'
labels: []
milestone: m-3
dependencies:
  - TASK-178
references:
  - TASK-88
  - TASK-178
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-178 scope-split follow-up; TASK-88 release-blocking child.** **43 original TASK-178 variants:** `predicates` (42) and `types` (1), exactly **559 observed parser/dialect cells** in `docs/task-178-full-variant-inventory.json`. Include integer intervals, Boolean and NULL truth, pattern/computed/temporal/environment predicates, and the conservative STRUCT CAST outcome. Own the typed/source/terminal semantic evidence, not external vendor execution. Vendor/session laws remain TASK-179, parser/Unsupported dispositions TASK-191, independent release-wide certification TASK-180. Original TASK-178 AC #1..#5 and TASK-88 AC #1..#8 remain binding at their mapped owner/final gate; completing this bounded task does not approve unrelated matrix rows, vendor runtime support or protocol 3.0.0. Source of truth: [scope split](../../docs/task-178-scope-split.json). Do not introduce parent-child cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded fixture IDs and evidence gates

- **fixture IDs:** `88-C192-P`, `88-C192-N` (positive and rejected/Impossible physical cases).
- **DuckDB / 13 dialect parser seam:** compare independent closed-source/terminal bags, complete emitted typed protocol outcomes and NULL/boundary evidence; never infer other vendor runtime execution.
- **Residual cases:** preserve explicit unapproved semantic gaps, required proof and downstream owner; never promote an unimplemented mandatory core variant to Exact or approved Residual.
- Freeze the task-owned matrix in `docs/coverage-manifest.json` before changing semantic code.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Freeze a task-owned v1 ledger for exactly the 43 original SQL variant IDs and their 13 parser dialects, with independent expected typed output (NULL, signed bounds, inclusivity/exclusivity), physical source-instance IDs, full selected/rejected/UNKNOWN or Impossible bags, and exactness classification. Register the ledger in `docs/coverage-manifest.json` before changing semantics.
- [ ] #2 Use public analyzer/configured-input APIs and independent DuckDB input/terminal goldens to validate strongest *sound* projected domains and TRUE/IS NOT TRUE/UNKNOWN partitions where executable; preserve duplicate occurrences, counterexamples, and explicit type-law assumptions. Do not equate parser acceptance or Unknown with certified exactness.
- [ ] #3 Cover the 43 named cases and their meaning-preserving dialect variants with explicit full-document outcomes and parser/Unsupported exceptions; preserve STRUCT CAST and vendor-dependent collation/timezone/NaN cases as named unapproved boundaries until their actual owners adjudicate them. Do not reclassify required supported behavior as Residual to pass.
- [ ] #4 Verify representable direct SQL/dbt/ODCS source facts affected by changes, then hand all positive/negative/conditional cell evidence to TASK-197 and TASK-180; pass exact-head format, lint, default/no-default Rust tests/docs, dbt E2E and pinned sql-tdg compatibility where relevant.
- [ ] #5 Complete exact-head rustfmt, strict Clippy, default/no-default Rust tests and docs, dbt E2E, active schema/goldens and relevant pinned sql-tdg consumer checks. Independently review every bounded acceptance cell and record the linked evidence in `docs/coverage-manifest.json`; TASK-88 parent and TASK-91 release remain unapproved until TASK-180 and separate parent signoff.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to TASK-193. TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.

## Incremental evidence (TASK-192 initial PR, 2026-11-10)

- Versioned [43-case/559-cell task-owned registry](../../docs/task-192-acceptance-freeze.json) preserves all original SQL and observed 13-dialect parser/canonical statuses. It explicitly records which expected semantic, source/terminal and adapter cells are still **not** certified; this registry is not a complete frozen executable acceptance matrix.
- Reuses 25 independent TASK-177 DuckDB TRUE/FALSE/UNKNOWN and terminal-bag oracles without copying their expectations. Adds four manually frozen complete-source nullable/duplicate string-pattern goldens for `LIKE`, `ILIKE`, `LIKE ... ESCAPE`, and prefix matching in [task_192_predicate_matrix.rs](../../tests/task_192_predicate_matrix.rs).
- The public analyzer/canonical-document checks must pass exact-head CI, and native/vendor collation, timezone, NaN, overflow, STRUCT and full-adapter expected-domain matrices remain open. **No AC #1..#5 is signed off by this increment.** Keep TASK-192 In Progress until every mandatory cell has independent exact expected outcome and full verification.
