---
id: TASK-128
title: Certify advanced frames vendor laws and TASK-74 signoff
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-22'
labels: []
milestone: m-3
dependencies:
  - TASK-127
references:
  - TASK-74
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-74 child 5/5 (originally 4/4).** Implement only certify advanced frames vendor laws and task-74 signoff in a focused PR against `main` after dependencies merge. The original TASK-74 acceptance criteria are additive and stay open until independently signed off. Never depend on TASK-74 itself.
<!-- SECTION:DESCRIPTION:END -->

## Finite preimplementation scope

- **Fixed fixture IDs:** `74-C128-P`, `74-C128-N` (positive and nonmembership/impossible case).
- **Variants to implement/classify:** NTILE; RANGE/GROUPS; COLLATE; NULL ordering; ranked_orders.sql; 13 dialects.
- **Independent concrete oracle baseline:** Oracle three ranking laws, one navigation, one ROWS frame, all five common aggregate-window families from TASK-189, grouped QUALIFY and explicit parser/session boundaries.
- **Fail-closed boundary and owner:** No exact claim for underordered or engine-dependent windows; parent signoff independent. TASK-128 owns every conditional variant and explicit review disposition.
- **Mapped original TASK-74 AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before semantic edits, freeze every named variant above and both IDs into an immutable task-owned fixture matrix in `docs/coverage-manifest.json`: exact SQL and parser status, canonical source/column identities, typed output domains including NULL/bound inclusivity, TRUE and NOT TRUE proof strength, expected full physical/terminal bags, dialect and adapter cells, and named conditional release disposition. No implicit exclusions or mandatory residuals.
- [x] #2 Implement the exact named core and its stated numerical/data oracle in the canonical protocol and physical source model. Explicitly preserve SQL three-valued truth, multiplicity, data/row identity, computed output-domain bounds and Exact/Impossible/Residual differences; prove full physical feasibility rather than parser or lineage only.
- [x] #3 Add public API integration tests with independent complete DuckDB source and terminal snapshots for both frozen IDs, including a NULL/duplicate/rejected or impossible counterexample. Assert entire bag, source identity and output value domain, not only row count or a generated snapshot.
- [x] #4 Check all 13 exposed parser dialect paths for semantically shared syntax, direct SQL and dbt compiled artifacts, and ODCS when equivalent schema evidence exists. Explicitly record parser/engine/session boundaries, conditional typed residuals and owner TASK-128; no claim of native engine execution without an executable oracle.
- [x] #5 On final PR head pass `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and schema/golden snapshots. Independently review every frozen mandatory cell and original AC mapping before marking this child Done and merging. Keep TASK-74 parent open.
<!-- AC:END -->

## Next

Then independently sign off all original TASK-74 AC against the entire merged child evidence.

## Verified implementation and independent signoff (2026-10-22)

- [PR #139](https://github.com/phdah/sql-semantic-protocol/pull/139) implements the canonical finite-bag NTILE and peer-aware RANGE/GROUPS laws, exact TRUE/NOT TRUE QUALIFY identities, typed value domains and fail-closed source/session classifications.
- [Versioned freeze v5](../../docs/task-128-acceptance-freeze.json): **5 mandatory Exact** complete source/terminal bags and **6 reviewed conditional Residual** boundaries, all **143** 13-dialect parser/canonical cells, all **33** three-adapter parity cells, typed NULL/multiplicity/interval domains and explicit TASK-128 conditional ownership. The two original P/N family IDs are retained.
- [Public physical/native oracles](../../tests/task_128_advanced_windows.rs): complete DuckDB bag comparison, 3 native conditional session-boundary bags, separate Impossible source contradiction, and exact/Residual proof strength. [Direct SQL/dbt/ODCS parity](../../tests/task_128_adapter_parity.rs) verifies every frozen case.
- [Real dbt Core E2E](../../tests/task_128_dbt_core_e2e.rs): executed seed/model for NTILE and GROUPS from compiled manifest/catalog and the pre-existing `ranked_orders.sql` full producer/terminal oracle. The previous project's semantic golden is preserved and checked independently.
- [Independent child and original TASK-74 AC audit](../../docs/task-128-acceptance-audit.md) reviews #1–#5 and maps all six parent criteria to their five child evidence sets. Non-DuckDB vendor/session cases remain **explicitly unexecuted Residual**, not false native support claims. They are conditional, not missing mandatory Exact coverage.
- **Semantic-head verification:** [all nine jobs passed on CI #3498](https://github.com/phdah/sql-semantic-protocol/actions/runs/39751701868): format, strict Clippy (both feature sets), full Rust tests (both feature sets), docs (both), dependency check, and real dbt Core E2E. Final documentation-only and acceptance-metadata head must also pass before merge.

**TASK-128 AC #1–#5: independently verified.** No mandatory exact cell remains unverified. Marked Done in this PR; do not merge until latest-head CI is green. TASK-74 stays **To Do** until this PR merges and a **separate** original six-AC parent signoff verifies all five merged children; TASK-91 remains release-blocked.

**PR #139 corrective review:** Versioned matrix v5 replaces the invalid N2 tie example with a genuine bucket-boundary counterexample, proves exact invariant-peer NTILE assignments, stores native conditional expected bags in the matrix, and verifies parsed conditional outcomes across all 13 dialects. Original exact cases and parent signoff gate remain unchanged.
