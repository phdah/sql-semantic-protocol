---
id: TASK-197
title: Close cross-adapter typed semantic parity for TASK-178 variants
status: To Do
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-10'
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

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Freeze a versioned task-owned cross-adapter ledger keyed by each original `88-C177-Snnn`/dialect identity and exact available source facts. Link it from `docs/coverage-manifest.json`. Every source-capable case must name direct SQL DDL, compiled dbt manifest/catalog and ODCS fixtures as applicable, plus reasons when an adapter cannot represent equivalent evidence.
- [ ] #2 Exercise public configured-input/composed/producer APIs with the strongest sound output value domains (NULL and inclusive/exclusive bounds), source-instance identities, lineage, proof strengths, complete terminal/after-state bags and required comparison assumptions. Assert equal full canonical facts when the same evidence exists; preserve distinct source provenance and unattested execution.
- [ ] #3 Audit all 163 variants, with explicit certified/conditional/unrepresentable status for every 13-dialect cell; fail on missing owned cell, unsupported feature silently accepted, adapter domain weakening, or mismatched physical source/writes. Do not turn an unavailable vendor/runtime law or parser rejection into an adapter parity success.
- [ ] #4 Reconcile the ledger with TASK-192..196 outcome goldens and pending TASK-179/TASK-191 boundary owners; hand an independently reviewable zero-omission matrix to TASK-180. Pass final-head full/no-default Rust, dbt E2E and pinned sql-tdg consumer compatibility checks.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to [TASK-179](task-179%20-%20classify-conditional-session-and-vendor-execution-laws.md). TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.
