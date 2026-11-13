---
id: TASK-193
title: Certify join and correlated source-instance bags across dialects
status: In Progress
assignee: []
created_date: '2026-11-10'
updated_date: '2026-11-13'
labels: []
milestone: m-3
dependencies:
  - TASK-192
references:
  - TASK-88
  - TASK-178
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-178 scope-split follow-up; TASK-88 release-blocking child.** **24 original variants:** `joins` (23) and `scopes` (1), **312 dialect cells**. Focus on NULL-safe, INNER/OUTER, SEMI/ANTI, theta and repeated-source joins, correlated LATERAL, shared physical input identity and full terminal bag multiplicities. The MSSQL `LATERAL ... ON TRUE` AST boundary remains owned by TASK-179/TASK-191 where native evidence is absent. Original TASK-178 AC #1..#5 and TASK-88 AC #1..#8 remain binding at their mapped owner/final gate; completing this bounded task does not approve unrelated matrix rows, vendor runtime support or protocol 3.0.0. Source of truth: [scope split](../../docs/task-178-scope-split.json). Do not introduce parent-child cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded fixture IDs and evidence gates

- **fixture IDs:** `88-C193-P`, `88-C193-N` (positive and rejected/Impossible physical cases).
- **DuckDB / 13 dialect parser seam:** compare independent closed-source/terminal bags, complete emitted typed protocol outcomes and NULL/boundary evidence; never infer other vendor runtime execution.
- **Residual cases:** preserve explicit unapproved semantic gaps, required proof and downstream owner; never promote an unimplemented mandatory core variant to Exact or approved Residual.
- Freeze the task-owned matrix in `docs/coverage-manifest.json` before changing semantic code.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Freeze task-specific v1 evidence for exactly 24 original variant IDs across all 13 parser dialects, with physical source-instance/alias identities and independent positive/rejected/Impossible/Residual source and terminal expectations.
- [ ] #2 Execute public API plus literal DuckDB goldens for complete input bags and multi-source terminal rows, including duplicate cross-products, NULL extension versus physical NULL, repeated aliases, anti/nonmembership and correlation. Enforce the strongest sound output domains, lineage and counts without decomposing a shared physical source into independent witnesses.
- [ ] #3 Assert canonical whole-document equivalence for supported shared SQL AST forms, retaining genuine dialect meaning differences and unexecuted vendors as explicit named boundaries. Specifically keep `88-C177-S152/mssql` unapproved until TASK-179/TASK-191 can resolve the required law.
- [ ] #4 Check source metadata equivalence from direct SQL/dbt/ODCS where it supplies the same fact; deliver cell-level results to TASK-197/TASK-180 and pass final-head repository CI and relevant sql-tdg consumer checks.
- [ ] #5 Complete exact-head rustfmt, strict Clippy, default/no-default Rust tests and docs, dbt E2E, active schema/goldens and relevant pinned sql-tdg consumer checks. Independently review every bounded acceptance cell and record the linked evidence in `docs/coverage-manifest.json`; TASK-88 parent and TASK-91 release remain unapproved until TASK-180 and separate parent signoff.
<!-- AC:END -->

## Next

After this task is independently verified, marked Done and merged, proceed to TASK-194. TASK-180 independently certifies the completed whole matrix and TASK-88 separately signs off the parent.

## Initial state

Extracted on 2026-11-10 from the previously overbroad TASK-178. There is **no implementation or semantic sign-off** for this ticket yet. Existing PR #197 supplies provisional structural observations and selected native oracles only; use those as regression inputs, not as proof of this ticket's acceptance.


## TASK-193 implementation evidence (2026-11-13, pending verification)

- Immutable task-owned 24-case/312-dialect preimplementation acceptance inventory: `docs/task-193-acceptance-freeze.json`; linked from `docs/coverage-manifest.json`. Original TASK-178 SQL spellings and parser masks are unchanged.
- Literal independently stated source-instance/NULL/multiplicity and TRUE versus NOT TRUE native oracles: `docs/task-193-source-instance-oracles.json`. Both physical sources use the original TASK-177 complete fixture.
- Test coverage: `tests/task_193_native_join_bags.rs` (24 original native outputs plus 11 occurrence-level proofs), `tests/task_193_dialect_semantics.rs` (312 full public-document comparisons), and `tests/task_193_adapter_parity.rs` (21 representable joins across SQL, dbt and ODCS typed source facts).
- S054 and S055 retain an explicit original DuckDB syntax rejection and separately identified native SEMI/ANTI equivalent; S152/MSSQL remains an unapproved canonical AST/session boundary for TASK-179/TASK-191. No native non-DuckDB execution or sql-tdg release is asserted.
- **Not signed off:** final-head fmt, Clippy, default/no-default feature suites, full Rust tests/docs, dbt E2E, pinned downstream consumer and independent two-axis review must be verified before AC #1..#5 can be marked complete. The parent TASK-88 and release TASK-91 remain open.
