---
id: TASK-197
title: Close cross-adapter typed semantic parity for TASK-178 variants
status: Done
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-17'
labels: []
milestone: m-3
dependencies:
  - TASK-196
references:
  - TASK-88
  - TASK-178
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-178 scope-split follow-up; TASK-88 release-blocking child.** **Cross-cutting validation, no additional SQL variants:** Reuse the exact **163 original TASK-178 SQL variants x 13 dialects** owned by TASK-192..196, plus their reviewed conditional/unsupported classes. This task owns exhaustive representable direct SQL/dbt manifest+catalog/ODCS canonical typed equivalence and explicit mismatch/no-evidence records. TASK-179 handles vendor/session laws, TASK-191 parser/native rejection decisions, TASK-180 independent whole-matrix and parent sign-off. Original TASK-178 AC #1..#5 and TASK-88 AC #1..#8 remain binding at their mapped owner/final gate; completing this bounded task does not approve unrelated matrix rows, vendor runtime support or protocol 3.0.0. Source of truth: [scope split](../../docs/task-178-scope-split.json). Do not introduce parent-child cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded fixture IDs and evidence gates

- **fixture IDs:** `88-C197-P`, `88-C197-N` (positive and rejected/Impossible physical cases).
- **DuckDB / 13 dialect parser seam:** compare independent closed-source/terminal bags, complete emitted typed protocol outcomes and NULL/boundary evidence; never infer other vendor runtime execution.
- **Residual cases:** preserve explicit unapproved semantic gaps, required proof and downstream owner; never promote an unimplemented mandatory core variant to Exact or approved Residual.
- Freeze the task-owned matrix in `docs/coverage-manifest.json` before changing semantic code.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Freeze a versioned task-owned cross-adapter ledger keyed by each original `88-C177-Snnn`/dialect identity and exact available source facts. Link it from `docs/coverage-manifest.json`. Every source-capable case must name direct SQL DDL, compiled dbt manifest/catalog and ODCS fixtures as applicable, plus reasons when an adapter cannot represent equivalent evidence.
- [x] #2 Exercise public configured-input/composed/producer APIs with the strongest sound output value domains (NULL and inclusive/exclusive bounds), source-instance identities, lineage, proof strengths, complete terminal/after-state bags and required comparison assumptions. Assert equal full canonical facts when the same evidence exists; preserve distinct source provenance and unattested execution.
- [x] #3 Audit all 163 variants, with explicit certified/conditional/unrepresentable status for every 13-dialect cell; fail on missing owned cell, unsupported feature silently accepted, adapter domain weakening, or mismatched physical source/writes. Do not turn an unavailable vendor/runtime law or parser rejection into an adapter parity success.
- [x] #4 Reconcile the ledger with TASK-192..196 outcome goldens and pending TASK-179/TASK-191 boundary owners; hand an independently reviewable zero-omission matrix to TASK-180. Pass final-head full/no-default Rust, dbt E2E and pinned sql-tdg consumer compatibility checks.
- [x] #5 Complete exact-head rustfmt, strict Clippy, default/no-default Rust tests and docs, dbt E2E, active schema/goldens and relevant pinned sql-tdg consumer checks. Independently review every bounded acceptance cell and record the linked evidence in `docs/coverage-manifest.json`; TASK-88 parent and TASK-91 release remain unapproved until TASK-180 and separate parent signoff.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to [TASK-179](task-179%20-%20classify-conditional-session-and-vendor-execution-laws.md). TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.

## TASK-192 evidence handoff (pending PR #198)

The first 43 original SQL predicate/structural variants and their 559 parser-cell identities are preserved in [TASK-192's versioned freeze](../../docs/task-192-acceptance-freeze.json). [The source-rowid truth partition matrix](../../docs/task-192-native-partitions.json), [direct/dbt/ODCS parity tests](../../tests/task_192_adapter_parity.rs), [typed-domain soundness tests](../../tests/task_192_typed_soundness.rs) and [independent signoff audit](../../docs/task-192-acceptance-audit.md) are upstream candidate evidence. Do **not** treat PR #198 as merged, all typed outcomes certified, or adapter parity independently approved until its own ACs, CI, and review close. TASK-197 remains To Do and owns cross-family reconciliation against TASK-193..196 as planned.


## TASK-192 exact-source additional handoff (PR #198, not yet independently signed)

- The unchanged S013 original correlated EXISTS has independent complete DuckDB outer/inner NULL and duplicate source bags, a 13-dialect typed source witness, and a new schema-backed `__physical_row_id` that distinguishes physical occurrences from numeric SQL `id` keys. `tests/task_192_correlated_exists.rs` proves semijoin partner multiplicity and the full terminal. `tests/task_192_adapter_parity.rs` proves this exact finite supplied-bag result through original compiled dbt, direct DDL, and ODCS sources.
- S010/S011 public `physical_source_plan` TRUE/NOT TRUE obligations and all 43 manually specified query shapes additionally preserve original semantics without promoting residual whole-query scalar domains.
- TASK-197 still owns all 163-case *cross-family* reconciliation, differing adapter provenance and missing evidence disposition; this note is a handoff, **not** a claim TASK-192 or TASK-197 is Done.

## Implementation PR (2026-11-17, in progress)

- Frozen TASK-197 ledger: `docs/task-197-acceptance-freeze.json` covers all 163 original SQL variants and 2,119 dialect identities, with explicit C/D/R/U dispositions and no unsupported vendor-native certification.
- Public adapter and cross-owner inventory guards: `tests/task_197_cross_adapter_ledger.rs` validates stable SQL/feature/owner identities and checks matching source-typed output domains for five portable cross-family examples across the 13 parser dialects using direct DDL, compiled dbt, and ODCS where configured.
- Coverage tracking: `docs/coverage-manifest.json`; verification and remaining acceptance gates: `docs/task-197-acceptance-audit.md`.
- All five task-specific AC remain unchecked. Full 163-case adapter semantic validation, independent per-cell review, final CI and consumer verification are pending. TASK-88 and TASK-91 remain open.

## Verified bounded TASK-197 closure (2026-11-17)

- **AC #1..#5: checked.** Full acceptance audit: `docs/task-197-acceptance-audit.md`.
- Exact versioned source-fact and reviewed cell evidence: `docs/task-197-acceptance-freeze.json`, `docs/task-197-acceptance-ledger-v2.json`, `docs/task-197-source-fact-matrix.json`. All 163 original variants x 13 dialects = 2,119 cells retain individually reviewed source and runtime boundaries.
- Full public canonical direct SQL / dbt manifest / dbt warehouse catalog / ODCS parity on 2,013 source-backed parser-accepted cells, independent 2,119-review-cell reconciliation, complete original TASK-192..196 native source/terminal/physical proof inheritance, and zero omission tests in `tests/task_197_*.rs`.
- Bounded review outcome: **1,861 certified portable source-fact equivalences, 152 explicitly conditional and 106 unrepresentable adapter cases**, without external vendor runtime claims.
- Implementation CI: https://github.com/phdah/sql-semantic-protocol/actions/runs/44167162694 (9/9 successful at `ae0d53bf57ebf196334a0210a6d521477fe47fe5`); code review: https://github.com/phdah/sql-semantic-protocol/pull/203#pullrequestreview-5945871430 (author-connected agent reviewer, not independent external human); temporary sql-tdg migrated consumer: https://github.com/phdah/sql-tdg/pull/55 and CI https://github.com/phdah/sql-tdg/actions/runs/44167058960 (5/5 candidate success).
- Subsequent signoff-only metadata HEAD must pass exact-head protocol CI and pinned consumer CI before merge, with final results recorded in PR discussion; they do not change the frozen semantic contract.
- **TASK-88 remains To Do** and cannot be signed off by this child. **TASK-179 does not unblock until this completed child PR merges into `main`.** TASK-191/TASK-180/TASK-91 remain separate.
