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


## Fourth pass nullable truth correction (PR #198)

- S016 numeric `IS TRUE` now retains the same-column `IS NOT NULL` guard
  to preserve FALSE rather than UNKNOWN for NULL source rows.
- Original S012/S016/S017 identities, all 13 dialects, typed Boolean witnesses,
  complete DuckDB truth partitions and negative DOUBLE source laws are tested.
- The original *fourth pass* attempted S011 NOT promotion failed seeded
  cross-conformance on NULL and nested predicate assumptions, so that broad
  promotion was reverted. The later ninth pass adds an explicit signed-integer
  type guard and precise SQL NULL-safe bound. Untyped S011 remains Residual.
- No independent reviewer approval is available for the 559 cell contract.
  Exact-head tests, migrated consumer pin, conditional case owners and remaining
  family residual resolutions all remain required before TASK-192 Done.

## Fifth pass bounded residual review evidence (PR #198)

- At the fifth pass, S010/S011 were both whole-query Residual but had complete signed source-local TRUE/NOT TRUE witnesses on all 13 parsers and direct/dbt/ODCS parity. The later ninth pass certifies **S011 only with authoritative signed-integer metadata** as Exact, preserving its untyped Residual baseline; S010 remains whole-query Residual.
- S027-S032 now have complete original native 3VL partition checks and explicit source-local typed outcomes. S028-S031 have exact operator-local witnesses. S027/S032 remain unapproved because arbitrary source integer values can trigger arithmetic overflow absent bounded source guarantees.
- Typed VARCHAR safe LIKE prefixes S020/S022/S025 and trailing-space equality S039 have conditional source-local witness tests requiring caller-attested binary collation and no padding. Fixed CHAR, ILIKE, regex, SIMILAR TO and locale-specific comparisons fail closed without native vendor law proof.
- The per-case and per-owner review route is in [independent TASK-192 handoff](../../docs/task-192-independent-review-handoff.md). The coverage manifest registers evidence without upgrading original public exactness, physical vendor certification or independent approval.
- The separate production [sql-tdg consumer migration draft](https://github.com/phdah/sql-tdg/pull/50) is awaiting independent review and a released protocol version. Test-only consumer PR #49 remains a temporary harness, not a merge path.
- The **559-cell ledger still requires a genuinely independent semantic reviewer**. Unapproved mandatory computed/relational/temporal/vendor/STRUCT residuals remain. All five ACs are unchecked, task status In Progress and parent/release signoff blocked.


## Sixth pass original S013 constructive closed-world proof

- `src/physical_realization/exists_bags.rs` now supports opt-in schema-backed nonempty unique text `__physical_row_id` as a complete physical source occurrence identity, distinct from an arbitrary numeric, nullable or duplicated SQL `id` key. Legacy textual `id` identity remains compatible. Contradictory duplicate physical identities produce Impossible, never false Exact.
- `tests/task_192_adapter_parity.rs` includes complete **S013** constructive semijoin source-bag parity for direct SQL, compiled dbt, and ODCS, using equivalent authoritative physical type and separate occurrence-identity facts. This must pass 13 parser names with native vendor execution explicitly unattested.
- `tests/task_192_correlated_exists.rs` independently executes the unchanged S013 SQL and its complement on a full orders/customers population with duplicated outer values, duplicated inner partners, unmatched IDs and SQL NULL keys. The public `physical_finite_exists_bag_plan` is checked across 13 original parsers for every matched/absent physical partner, selected/NOT TRUE outer occurrence, complete terminal `id` bag and finite **supplied-bag** typed domain. This does not convert the global open-source `id` range into a finite unconditional domain.
- `docs/task-192-original-query-shapes.json` and `tests/task_192_independent_query_shapes.rs` add 43 original SQL-anchored manually specified expected outer source, output and normalized WHERE operator shapes, including S179 two parser rejects. This addresses first-analyzer-as-sole-expected-shape weakness but **does not** replace independently approved 43 full-document expected goldens.
- AC #1..#5 remain unchecked pending final typed full-document proof, remaining mandatory portable cases, and actual distinct 559-cell reviewer approval.

## Seventh pass source-level physical and signed overflow evidence

- Original S010/S011 now additionally assert public `physical_source_plan` **constructible individual physical source-row TRUE/NOT TRUE** obligations for all 13 dialects, without promoting whole-query independent-scalar exactness or unbounded projected IDs. This complements the existing original full 3VL DuckDB source/terminal partitions.
- Original S027 and S032 now run the **unaltered** addition and ABS SQL on separate signed BIGINT extrema plus NULL. DuckDB successfully evaluates S027 BIGINT::MAX + 2 by using wider arithmetic; S032 tests the distinct ABS(BIGINT::MIN) failure boundary. This independently demonstrates engine-specific width laws and why DuckDB behavior cannot be assumed portable without source/result-width proof.
- The S013 finite original semijoin is now additionally verified as an exact complete physical bag across direct SQL, compiled dbt and ODCS (39 parser-adapter combinations with ODCS enabled, 26 without). Every adapter retains producer-execution Unattested.
- These checks are in-scope evidence, **not** an independent external semantic reviewer decision, a full 43-case authored wire golden or a release-vendor approval. Do not check any of the five task ACs until that gate is met.


## Ninth pass: genuine original S011 portable semantic closure

- **Closed:** original S011 `NOT (amount > 1)` previously had no source-independent scalar-range proof, even with a typed signed `orders.amount`. `src/analysis/exactness.rs::typed_signed_ordered_not` now admits only ordinary direct source-column signed comparisons, numeric integer literals and an **authoritative signed source datatype**. `src/analysis/domains.rs` derives `amount <= 1`, inclusive, excluding NULL, without restricting the unrelated projected `id`.
- **Fail-closed:** no type evidence, floating/NaN, text/coercion, nested NOT, computed arithmetic, and other unsupported patterns do not borrow the signed trichotomy law. `src/domain.rs` separately supports ordinary direct `NOT (=)` / `NOT (<>)` integer literal duals, which are safe under SQL 3VL and preserve NULL UNKNOWN. This additional generic case is not a substitute for any original 43-case oracle.
- `tests/task_192_not_equality.rs` checks all 13 parser dialects, independent DuckDB TRUE/FALSE/UNKNOWN/NOT TRUE source occurrence bags, 3VL nullable duplicates, exact signed bound and negative FLOAT/untyped cases. `tests/task_192_nullable_truth.rs` now distinguishes S010 cross-column Residual from S011 **catalog-typed** Exact while preserving physical TRUE/NOT TRUE proofs for both.
- **Important:** `docs/task-192-semantic-cell-ledger.json` is the *untyped* v1 public baseline, so its original S011 untyped classification remains Residual. Separate configured source evidence proves S011 typed Exact. Neither baseline nor typed evidence has an independent reviewer approval; the five ACs remain unchecked and TASK-192 stays In Progress.

## Additional S010 typed Boolean implementation and review readiness (2026-11-12)

- Original S010 `amount > 1 OR id > 0` now certifies its full same-row
  TRUE/NOT TRUE Boolean condition under **one authoritative signed-integer
  source**, without inventing an independent finite output `id` range.
  Untyped, DOUBLE and multi-source cases remain conservative. Original S010
  13-cell untyped ledger and the separate conditional/vendor boundaries
  are unchanged.
- Original S010 signed positive and FLOAT negative regression coverage lives
  in `tests/task_192_nullable_truth.rs` alongside the complete DuckDB
  partitions and direct/dbt/ODCS parity. Conservative 559-cell literal/domain,
  named residual identity, conditional comparison-law and query-shape
  assertions remain in their task-owned public tests.
- `docs/task-192-review-queue.json` explicitly queues all 559 original
  review decisions. `tests/task_192_review_queue.rs` checks the review list
  is complete and remains unsigned pending separately accountable review.
- CI now explicitly provisions pinned DuckDB 1.5.6 shared libraries before
  native Rust/build jobs instead of relying on transient build-script
  download path inference.

**AC #1..#5 are still unchecked.** Independent full-document goldens, a
distinct reviewer's signoff of all 559 original cells, legitimate residual
dispositions and exact-head protocol/consumer CI are outstanding. Do not
mark TASK-192 Done or PR #198 ready to merge on author-side CI alone.


## Additional complete candidate wire regressions (2026-11-12)

- `docs/task-192-proposed-full-documents.json` now pins a complete
  analyzer-produced canonical-document candidate for **every original 43-case
  SQL**, preserving integer widths exactly. `tests/task_192_full_document_goldens.rs`
  asserts all fields for every accepted 13-dialect parser cell (557 successes,
  2 original S179 parser rejections); no longer uses the first analyzer output
  as a transient regression oracle.
- The separate frozen manually specified SQL shapes, scalar range/set/null
  outcomes, full DuckDB truth partitions and source/terminal bags remain the
  independent semantic evidence. The full-document candidates themselves
  still require **distinct independent human review**, especially where a
  semantic mistake might be stable across all dialects.
- `docs/task-192-review-queue.json` links those 43 complete candidates
  without manufacturing any of the 559 independent review decisions. No
  release or TASK-192 AC box has been signed by generating this fixture.


## Bounded original semantic closures: S013, S027 and S032

- `docs/task-192-bounded-semantic-closures.json` records the precise
  physical evidence contract and necessary residual bounds for all three.
- **S013:** original correlated EXISTS has exact complete two-source
  selected/rejected/NULL/duplicate bag; a new negative test verifies
  *missing inner population* is Residual and *attested empty inner*
  is Exact without fabricating matches.
- **S027/S032:** new canonical Rust API
  `physical_finite_signed_computed_bag_plan` evaluates source-local
  addition/ABS for exact signed widths 16/32/64, using complete source
  snapshots and checked result-width bounds. All 13 parser dialects
  are compared to native original TRUE/FALSE/UNKNOWN/NOT TRUE
  source/terminal goldens; duplicate occurrence and finite typed
  terminal set/bounds are checked. Signed extrema with possible
  overflow return a named Residual and no false portable Exact.
- This closes the **bounded physical construction** implementation
  gap without changing global untyped open-world classifications.
  Independent reviewer signoff, stronger universal/vendor law decisions
  if required, and 559-cell provenance still block Done.
