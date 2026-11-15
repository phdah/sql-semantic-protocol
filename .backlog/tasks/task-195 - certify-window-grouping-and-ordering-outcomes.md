---
id: TASK-195
title: Certify window, grouping, ordering and pagination outcomes
status: In Progress
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-10'
labels: []
milestone: m-3
dependencies:
  - TASK-194
references:
  - TASK-88
  - TASK-178
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-178 scope-split follow-up; TASK-88 release-blocking child.** **36 original variants:** `grouping` (3), `windows` (19), and `ordering` (14), **468 dialect cells**. Includes ROLLUP/CUBE/GROUPING NULL versus subtotal, rank/peer/frame and QUALIFY semantics, TOP/FETCH/LIMIT/offset ordering, NULL sort and nondeterministic sampling boundaries. Parser dialect equality does not prove warehouse ordering/session behavior. Original TASK-178 AC #1..#5 and TASK-88 AC #1..#8 remain binding at their mapped owner/final gate; completing this bounded task does not approve unrelated matrix rows, vendor runtime support or protocol 3.0.0. Source of truth: [scope split](../../docs/task-178-scope-split.json). Do not introduce parent-child cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded fixture IDs and evidence gates

- **fixture IDs:** `88-C195-P`, `88-C195-N` (positive and rejected/Impossible physical cases).
- **DuckDB / 13 dialect parser seam:** compare independent closed-source/terminal bags, complete emitted typed protocol outcomes and NULL/boundary evidence; never infer other vendor runtime execution.
- **Residual cases:** preserve explicit unapproved semantic gaps, required proof and downstream owner; never promote an unimplemented mandatory core variant to Exact or approved Residual.
- Freeze the task-owned matrix in `docs/coverage-manifest.json` before changing semantic code.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Freeze the 36 original variants x 13 parser dialects with independent closed input/output bags, precise rank/group/frame boundary expectations, typed domains, stable identities, ties and NULL/duplicate evidence, and named unsupported/parser/engine cases.
- [ ] #2 Verify through public APIs and DuckDB complete source/terminal oracles the sound grouped/global empty-input result, physical NULL versus generated subtotal, ranks and frames, QUALIFY positive/rejected rows, total/partial order, peer boundaries and pagination count/multiplicity. Assert strongest sound output facts; unstable tie order must not become Exact.
- [ ] #3 Keep `FETCH WITH TIES` distinct from LIMIT, recognize only independently equivalent TOP/FETCH/LIMIT variants, and preserve sampling/NULL ordering/session-dependent outcomes as explicit conditional non-certifications owned by TASK-179/TASK-191.
- [ ] #4 Verify all 13 parser statuses and source adapters where comparable facts exist, hand cells to TASK-197/TASK-180 and pass the full final-head repository and pinned downstream gates.
- [ ] #5 Complete exact-head rustfmt, strict Clippy, default/no-default Rust tests and docs, dbt E2E, active schema/goldens and relevant pinned sql-tdg consumer checks. Independently review every bounded acceptance cell and record the linked evidence in `docs/coverage-manifest.json`; TASK-88 parent and TASK-91 release remain unapproved until TASK-180 and separate parent signoff.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to TASK-196. TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.

## TASK-195 implementation work (open PR, not signed off)

- Preimplementation task-owned 36 original SQL / 468 parser-cell inventory: `docs/task-195-acceptance-freeze.json` and `docs/coverage-manifest.json`.
- Native original source/terminal regression and new independently worked numeric window/frame examples: `tests/task_195_native_window_bags.rs`.
- Public full-document classification and exact SQL/case/cell identity regression: `tests/task_195_acceptance_matrix.rs`.
- Remaining before Done: independent strongest-typed-domain certification, exact source-instance positive/NOT TRUE/negative proof where applicable, SQL/dbt/ODCS parity, full CI and pinned sql-tdg verification, plus independent review of all applicable cells. Keep TASK-179, TASK-191, TASK-197 and TASK-180 gates separate; do not treat C parser classification as semantic proof.

## TASK-195 extended evidence on PR #201 (signoff pending)

- Native original terminal/domain soundness: `tests/task_195_typed_soundness.rs`; direct/dbt/ODCS original SQL evidence: `tests/task_195_adapter_parity.rs`.
- Source occurrence, explicit empty source group/ROLLUP, NULL sort boundaries: `tests/task_195_physical_source_instances.rs`.
- Original QUALIFY positive vs NOT TRUE populations, stochastic source-subset checks: `tests/task_195_native_partitions.rs`.
- Complete individual outstanding review obligations: `docs/task-195-review-queue.json`; AC-by-AC assessment: `docs/task-195-acceptance-audit.md`.
- No independent 468-cell review, full strongest-domain/physical-proof signoff or exact-head downstream consumer evidence has yet been verified. Do not mark task Done on CI alone.
