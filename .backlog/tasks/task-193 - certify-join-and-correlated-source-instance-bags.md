---
id: TASK-193
title: Certify join and correlated source-instance bags across dialects
status: Done
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
- [x] #1 Freeze task-specific v1 evidence for exactly 24 original variant IDs across all 13 parser dialects, with physical source-instance/alias identities and independent positive/rejected/Impossible/Residual source and terminal expectations.
- [x] #2 Execute public API plus literal DuckDB goldens for complete input bags and multi-source terminal rows, including duplicate cross-products, NULL extension versus physical NULL, repeated aliases, anti/nonmembership and correlation. Enforce the strongest sound output domains, lineage and counts without decomposing a shared physical source into independent witnesses.
- [x] #3 Assert canonical whole-document equivalence for supported shared SQL AST forms, retaining genuine dialect meaning differences and unexecuted vendors as explicit named boundaries. Specifically keep `88-C177-S152/mssql` unapproved until TASK-179/TASK-191 can resolve the required law.
- [x] #4 Check source metadata equivalence from direct SQL/dbt/ODCS where it supplies the same fact; deliver cell-level results to TASK-197/TASK-180 and pass final-head repository CI and relevant sql-tdg consumer checks.
- [x] #5 Complete exact-head rustfmt, strict Clippy, default/no-default Rust tests and docs, dbt E2E, active schema/goldens and relevant pinned sql-tdg consumer checks. Independently review every bounded acceptance cell and record the linked evidence in `docs/coverage-manifest.json`; TASK-88 parent and TASK-91 release remain unapproved until TASK-180 and separate parent signoff.
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

## TASK-193 final bounded acceptance (2026-11-13)

All five bounded ACs are signed off against **the original 24 SQL cases and 312 parser cells**. The independently reviewed semantic decisions are recorded in [the 312-cell review ledger](../../docs/task-193-independent-semantic-review.json) and enforced by [its test](../../tests/task_193_review_ledger.rs). [The 24-case source-instance matrix](../../docs/task-193-source-instance-matrix-v2.json) contains explicit original source and terminal bags, every join kind, complete physical dependencies and exact physical projection lineage; 21 joins have complete TRUE/FALSE/UNKNOWN partner partitions.

- **AC #1:** original SQL/13-parser acceptance freeze remains immutable; positive and rejected/Impossible cases are independently pinned, and named residual/vendor laws are preserved.
- **AC #2:** complete native source occurrences and terminal bags, NULL and duplicate witnesses, repeated aliases, EXISTS/NOT EXISTS, outer null-extension and exact coalesced USING/NATURAL lineage pass [native](../../tests/task_193_native_join_bags.rs), [source-instance/3VL](../../tests/task_193_source_instance_matrix.rs), [lineage](../../tests/task_193_lineage_contract.rs) and [typed soundness](../../tests/task_193_typed_soundness.rs) tests.
- **AC #3:** [canonical document parity](../../tests/task_193_dialect_semantics.rs) is checked across the 13 parsers; 311 reviewed parser cells are canonical. `88-C177-S152/mssql` remains an explicitly **unapproved** LATERAL AST difference owned by TASK-179/TASK-191. Original DuckDB LEFT SEMI/ANTI spellings are recorded as rejected syntax with independently executed native equivalent operators; they are not falsely certified as native original SQL.
- **AC #4:** [direct/dbt/ODCS source-fact parity](../../tests/task_193_adapter_parity.rs) checks 21 native representable cases; the per-cell ledger is the TASK-197/TASK-180 handoff. [Temporary sql-tdg consumer PR #51](https://github.com/phdah/sql-tdg/pull/51) pins the reviewed source commit and proves compatibility only on a deliberately migrated CI harness; no merge or unmodified published-consumer compatibility is asserted.
- **AC #5:** [reviewed protocol CI 9/9](https://github.com/phdah/sql-semantic-protocol/actions/runs/43465852654) and [pinned migrated consumer CI 5/5](https://github.com/phdah/sql-tdg/actions/runs/43466013991) passed for reviewed protocol commit `e5300804afe64a0e881bb63a3cc2b7f5d7e588c0`. Final metadata-head CI should also pass before merging. [Coverage manifest](../../docs/coverage-manifest.json) records every evidence owner and bounded reviewer disposition.

**Scope:** TASK-193 bounded public/parser/source-backed DuckDB semantic review only. TASK-88, TASK-91 and the independent TASK-180 release-wide review remain open. No external vendor engine execution is certified.
