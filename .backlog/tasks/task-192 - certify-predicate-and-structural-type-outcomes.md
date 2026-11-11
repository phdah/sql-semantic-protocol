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

## Expanded original-case evidence (PR #198)

- [TASK-192 native source-instance matrix](../../docs/task-192-native-partitions.json) adds 11 closed-world TRUE/FALSE/UNKNOWN/NOT TRUE rowid partitions against pinned full DuckDB sources, including timezone/DST, collation, regex, NaN and signed-zero cases. Separate pinned original terminal bags are replayed by [native tests](../../tests/task_192_native_partitions.rs).
- The remaining three non-predicate cases have separately typed original DuckDB outcomes: arithmetic overflow is an evaluation error rather than invented output; ORDER BY NULLS LAST preserves a complete six-row ordered bag; STRUCT exposes one independently verified {a=1,b='a'} scalar without claiming portable typed struct support.
- [Adapter source-fact parity tests](../../tests/task_192_adapter_parity.rs) cover representable direct SQL, compiled dbt, and ODCS metadata for the 41 source-bearing original SQLs × 13 dialects. These tests do not claim that dbt or other vendors executed the SQL.
- [Acceptance audit](../../docs/task-192-acceptance-audit.md) maps each AC #1..#5 to its currently verifiable evidence and outstanding signoff blockers. Strongest universal typed domains, complete expected per-dialect semantics, pinned downstream compatibility and independent reviewer approval remain blocking. **All task ACs stay unchecked** until fully certified.

- [Typed domain soundness tests](../../tests/task_192_typed_soundness.rs) replay all 41 successful original scalar native terminal bags against all 13 parser output domains and verify that the 31 nullable `id` projections never acquire an invented finite range or inclusion set from an unrelated predicate. Ten exact portable signed-bound/set claims are sourced from independent original predicate goldens and explicitly frozen in the TASK-192 matrix; full independent per-cell certification is still outstanding.
- [Temporary sql-tdg consumer PR #49](https://github.com/phdah/sql-tdg/pull/49) pins a protocol candidate for downstream compatibility testing; it is a CI-only verification branch and must never be merged. Update the pin to exact final protocol head before recording signoff.

## Second pass implementation evidence (PR #198)

- [559-cell semantic contract](../../docs/task-192-semantic-cell-ledger.json) and [public-API test](../../tests/task_192_semantic_contract.rs) now guard expected exact/conditional/residual, projected domain kind, named residual reasons, parser rejection and historic repaired STRUCT drift on every SQL/dialect cell.
- [dbt adapter parity](../../tests/task_192_adapter_parity.rs) analyzes the original compiled SQL instead of unrelated placeholder SQL for all 41 source-bearing cases in all 13 public parsers.
- [Alternate native source counterexamples](../../tests/task_192_projection_counterexamples.rs) exercise all 3VL partitions with independent NULL/large/negative/duplicate projected IDs in string/temporal/NaN query families.
- Original S018 nullable numeric `IS UNKNOWN` normalizes to an exact `IS NULL` predicate at the public seam, preserving positive and NOT TRUE physical semantics. S012 nullable interval disjunction and S017 logical NOT semantics are still explicit mandatory semantic limitations. **They are not approved Residual dispositions.**
- Full independent reviewer signoff, mandatory sound physical realization for remaining residual variants and final-head pinned sql-tdg migration still block Done; keep all AC boxes unchecked pending proof.

## Third pass nullable 3VL repairs (PR #198)

- [S012/S017 native truth cases](../../tests/task_192_nullable_truth.rs) independently compare complete original TRUE/FALSE/UNKNOWN/NOT TRUE source and terminal bags with canonical matching-column Boolean trees, including NULL exclusion, duplicate rows, signed boundaries and negative controls.
- The public analyzer now retains matching numeric comparison `OR IS NULL` as an exact Boolean witness without fabricating scalar `ValueDomain::Ranges + NULL`. An Unknown source-domain encoding cannot override this exactness when it has no independent type failure.
- Numeric `IS FALSE` has an explicit `NOT comparison AND column IS NOT NULL` normalization, preserving FALSE on SQL NULL without replacing NOT with an unsafe comparison complement. Untyped plain NOT and unrelated OR remain explicit residuals.
- [559-cell expected semantic ledger](../../docs/task-192-semantic-cell-ledger.json) now records S012/S017 as Exact on all 13 parser dialects; external vendor/session claims remain unattested.
- Full original-task independent semantic reviewer approval, relevant conditional owner dispositions, and final pinned consumer migration approval are still prerequisites for checking AC #1..#5 and marking Done.

## Third pass source-type and adapter safety

- [Typed source-law negative proof](../../tests/task_192_nullable_truth.rs) checks all 13 parsers with original S012/S017 SQL against an unattested DOUBLE source; no exact signed integer row-membership proof may be inferred through NaN/signed-zero semantics. Parser-only Exact is a symbolic SQL condition shape, not vendor runtime certification.
- [Adapter 3VL semantic parity](../../tests/task_192_adapter_parity.rs) now compares direct, original compiled dbt and ODCS complete Boolean witness tree, TRUE/NOT TRUE directions and condition-exactness strength for both named nullable cases on all 13 parsers. Keep producer execution `unattested`.


## Fourth pass exact nullable truth corrections (PR #198)

- S016 numeric comparison `IS TRUE` now retains a same-column `IS NOT NULL` guard so its full FALSE/UNKNOWN/NOT TRUE partition matches the original SQL.
- S011 ordinary signed-integer comparison `NOT` now has a narrowly permitted exact Boolean witness without upgrading arbitrary logical NOT or floating sources.
- The original S011/S012/S016/S017 source identities, all 13 public parsers, direct typed Boolean witnesses and unattested FLOAT negative laws have regression coverage.
- The 559-cell expected ledger reflects the new S011 classification; all four cases remain **unreviewed for release**. Remaining mandatory family residuals, independent review, the exact final-head consumer pin and CI continue to block all five acceptance criteria.
