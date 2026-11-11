# TASK-192 predicate and structural-type acceptance audit

Status: **In Progress, mandatory global signoff blocked.** This file records the
bounded evidence on PR #198 without upgrading prior parser observations into
native engine certification. Original TASK-178 and TASK-88 acceptance criteria
are unchanged.

## Source of truth

- **43 original SQLs, 559 parser/canonical cells:** `docs/task-178-full-variant-inventory.json`.
- **Task-owned v1 inventory and missing-evidence fields:** `docs/task-192-acceptance-freeze.json`. Original SQL, stable IDs, and historical observed dialect statuses remain immutable.
- **40 complete predicate TRUE/FALSE/UNKNOWN DuckDB source partitions:** 25 previously independently pinned in `docs/task-177-predicate-oracles.json`, four new pattern controls in the TASK-192 freeze, and eleven newly frozen source-rowid partitions in `docs/task-192-native-partitions.json`.
- **Three non-predicate controls:** independent native overflow evaluation error S033; ordered NULL/tie terminal S042; STRUCT(a=1,b='a') scalar S179. No fabricated TRUE/FALSE partition for these non-predicate outcomes.
- **Public tests:** `tests/task_192_predicate_matrix.rs`, `tests/task_192_native_partitions.rs`, `tests/task_192_adapter_parity.rs`. Earlier separate TASK-177 native tests continue to verify the exact heterogeneous complete source values, not just row counts.

## Acceptance mapping

| Criterion | Evidence added to PR | Remaining signoff blocker |
| --- | --- | --- |
| AC #1: finite 43 × 13 versioned matrix | Complete original IDs, parser cells, linked native oracles with source-instance rowids and explicit missing facts. | Independent expected strongest typed domains and exact per-dialect semantic/oracle cells are not all frozen. Parser-equivalent C is not a semantic proof. |
| AC #2: public analyzer, exact DuckDB physical bags and type laws | 40 original predicate TRUE/FALSE/UNKNOWN/NOT TRUE closed-world source partitions, a rejected overflow original, NULL order and STRUCT scalar controls; 13-dialect public document equivalence. | End-to-end constructive physical proof and strongest safely provable typed domains are not yet independently certified for every SQL. |
| AC #3: named families, full wire outcomes, parser boundaries | All 43 original SQL records and 559 observed canonical cells retained; STRUCT parser exceptions are explicit; original native scripts are executed rather than synonyms. | No global independent expected-document semantic review; vendor collation, timezone, NaN and structural type laws are conditional. |
| AC #4: source adapter facts / handoff | Direct SQL, dbt compiled/source metadata, ODCS source-schema parity tests for 41 source-bearing queries × 13 parser dialects; no artificial adapter claim for overflow/STRUCT scalars. | Final TASK-197 cross-family parity handoff and pinned sql-tdg consumer compatibility remain unverified. |
| AC #5: final-head CI and independent review | GitHub Actions executes fmt, strict Clippy, default/no-default Rust tests/docs and dbt E2E. | Require final-head green CI, independent cell-by-cell review, active schema/golden checks, and relevant pinned consumer evidence before Done. |

## Soundness boundaries and release ownership

A source bag and a selected terminal are distinct from a general domain proof:
the test input bag does not constrain arbitrary future relation data. When a
query projects `id` but only filters `name`, `amount` or `created_at`,
a value restriction on the filtered source column must **not** be misapplied
as an `id` domain. Nullability, numeric widths, signed range endpoints and
exclusion directions must be checked from typed public outcome evidence.
Unknown is not Exact, and an accepted parse is not native execution.

The original S033 fixed-width overflow has no successful terminal row. S042
uses a fixed DuckDB NULLS LAST session and a complete ordered terminal, without
inventing stable identities for underordered peers. S179 has a positive native
STRUCT scalar and explicit nonportable parsing/typed-output boundaries.
S026/S038 locale-aware string comparisons, S035/S037 timezone/DST boundaries,
and S040/S041 NaN/signed zero preserve the actual native DuckDB evidence only.

- **TASK-179** owns external vendor/session laws; no approval is implied here.
- **TASK-191** owns parser/Unsupported dispositions, notably STRUCT.
- **TASK-197** owns independent cross-adapter parity handoff.
- **TASK-180** owns independent whole-matrix certification.
- **TASK-88/TASK-91** remain unsigned release gates.

No missing mandatory core semantic behavior is an approved Residual.
Only mark TASK-192 Done after every AC #1..#5 has direct executable evidence
and independent verification against the frozen SQL, type, source and terminal
matrices, plus passing exact-head CI.

## Additional typed-policy evidence

`docs/task-192-acceptance-freeze.json` now assigns every original variant an independent expected-output policy: 10 exact scalar set/interval and bound-direction examples (S001–S009 and S012), 31 unconstrained nullable `id` projections, one native fixed-width overflow error (S033), and one conservative STRUCT CAST boundary (S179). These policies cover all 559 parser cells as an invariant floor, but parser coverage must not be confused with independently certified strongest typed semantics.

`tests/task_192_typed_soundness.rs` checks that all 41 successful original numeric terminals fit the emitted domains on every parser, including NULLs, duplicates and signed values, and ensures no unsupported finite `id` restriction is inferred from predicates on other source columns. The test is evidence of soundness on the pinned native samples, **not** completeness over arbitrary source bags.

The temporary [sql-tdg consumer compatibility PR #49](https://github.com/phdah/sql-tdg/pull/49) has an exact protocol commit pin for CI only. The main consumer's published dependency must remain unchanged.

## Second implementation pass: exact public semantics and source-independent counterexamples

- `docs/task-192-semantic-cell-ledger.json` now freezes all 559 named public analyzer cells with explicit expected projected-domain kind, exact/conditional/residual status, named residual cause, parser-rejection/STRUCT-drift adjudication and a reference to their physical DuckDB evidence. It does **not** copy generated output as an independently verified full-document golden.
- `tests/task_192_semantic_contract.rs` enforces every named cell through `analyze_sql` + canonical `to_json`, checking all expected status fields, original case references, explicit parser failures, and cross-dialect full-document equivalence. Ten precise numeric cases cross-reference independent bound/set/inclusivity fixtures. Historic DuckDB STRUCT D is repaired; BigQuery and generic parser rejections remain explicit.
- `tests/task_192_adapter_parity.rs` now analyzes **the original SQL** as each compiled dbt model, not a dummy SELECT, and checks composed results against direct configured-input and exported source schemas for all 41 applicable original cases × 13 dialects. ODCS type-source parity remains separate. No producer metadata attests actual vendor execution.
- `tests/task_192_projection_counterexamples.rs` tests new complete source and terminal DuckDB bags with deliberately negative, NULL, large and repeated output `id` values for original pattern/temporal/NaN predicates. These are counterexamples to accidentally deriving finite projected ID membership from predicates on unrelated columns.
- `src/analysis/predicates.rs` adds a narrow, truth-preserving rule for `(nullable_column > numeric_literal) IS UNKNOWN`: the test is exactly `nullable_column IS NULL` (for TRUE, FALSE and NOT TRUE). It does not generalize through casts, arithmetic, functions or null-safe comparisons. The public regression test enforces this for original S018 on 13 dialect parsers.

### Third implementation pass: S012/S017 complete 3VL boundary

- The `tests/task_192_nullable_truth.rs` native oracles check the unchanged S012 and S017 SQL against separately authored, fully populated DuckDB source bags and the complete original TRUE/FALSE/UNKNOWN/NOT TRUE row identities. No UNKNOWN row is silently discarded. The typed public configured-input test requires **both Exact TRUE and Exact NOT TRUE** Boolean-witness directions on the 13 registered parsers.
- S012 preserves the original `(amount > 1 OR amount IS NULL) AND id > 0` as a coupled Boolean tree. `src/analysis/exactness.rs` distinguishes an unrepresentable *scalar column-domain encoding* from a truly unsupported Boolean condition: only a matching numeric comparison plus same-column IS NULL can carry an exact tree, and only if there is no separate typing error. It must not invent a finite `range+NULL` scalar domain. The output `id > 0` signed and exclusive lower bound remains independently pinned.
- S017 now normalizes an ordinary numeric `column <op> literal IS FALSE` to `NOT (column <op> literal) AND column IS NOT NULL`; the latter rejects NULL as FALSE, not UNKNOWN, preserving the entire outer test. This does **not** invert the numeric comparison itself, so it does not assume binary complements for NaN. Plain `NOT` and cross-column `OR` retain explicit residual outcomes.
- The 559-cell ledger pins the new **exact symbolic Boolean classifications** for both original cases over 26 public parser cells, without modifying original SQL, native oracle IDs or vendor attestations. The active protocol contract explains both truth laws. Any future branch that removes either NULL guard or invents a finite output `id` set fails explicit regression tests.
- A separate 26-cell negative type-law test configures `orders.amount` as DOUBLE while leaving NaN and signed-zero comparison settings unattested. `src/analysis/exactness.rs` must not suppress an Unknown-domain residual under floating or other unverified source types; only missing type metadata or independently declared signed integer types can follow the symbolic nullable proof. Neither public parser acceptance nor an integer-only DuckDB oracle certifies the FLOAT case.
- The original compiled dbt and ODCS parity tests now compare **entire coupled Boolean witness constraints, qualifying/rejected directions and row-condition exactness**, not just output domains, for S012 and S017 on all 13 parsers. dbt producer execution remains separately unattested.
- S033 and S179 have `condition_exactness=exact` only because no filtering WHERE clause exists. Their **output value domains remain Unknown** and do not certify a successful native overflow or portable nested STRUCT CAST.

### Fourth implementation pass: S016 nullable IS TRUE repair

- **S016** previously mapped `amount > 0 IS TRUE` to `amount > 0`,
  incorrectly turning a FALSE outcome on NULL source rows into UNKNOWN.
  `src/analysis/predicates.rs` now retains
  `amount > 0 AND amount IS NOT NULL`, preserving the original complete
  TRUE/FALSE/UNKNOWN/NOT TRUE source partition.
- `tests/task_192_nullable_truth.rs` exercises the unchanged S012/S016/S017
  DuckDB source and terminal oracles, 39 public parser cells, typed exact
  TRUE/NOT TRUE Boolean witnesses, and 39 unattested DOUBLE negative controls.
- An attempted S011 numeric NOT exactness promotion failed independently
  seeded conformance in `tests/differential_conformance.rs`: NULL-membership
  assumptions and nested CTE predicate analysis were insufficient. This
  promotion was reverted. S011 remains the **unapproved logical_not residual**
  in all 13 dialect cells, with a fail-closed negative regression.
- The versioned 559-cell ledger preserves the existing source and terminal
  scope, exact S012/S016/S017 classification, and explicit unreviewed flags.

The verified S016 correction does not replace independent reviewer validation,
approve remaining mandatory residuals or demonstrate unmodified consumer
compatibility.

### Fifth pass: independently replayable residual evidence, without release approval

- S010/S011: [typed Boolean source-row witness tests](../tests/task_192_nullable_truth.rs) now replay original full DuckDB TRUE/FALSE/UNKNOWN/NOT TRUE populations and verify the source-typed exact row-local Boolean witness on all 13 parser dialects. [Adapter parity](../tests/task_192_adapter_parity.rs) also checks direct, original compiled dbt and ODCS semantics for these original cases. Neither is a whole-query independent-domain proof. The two residual classes remain explicitly unapproved.
- S027-S032: [computed predicate evidence](../tests/task_192_computed_witness.rs) checks each original whole-bag native result and every three-valued partition. Source-typed exact row-level witnesses are demonstrated for S028-S031. Arithmetic addition and absolute-value cases S027/S032 remain residual without a complete source range and overflow proof.
- S020/S022/S025 and S039: [pattern law evidence](../tests/task_192_string_witness.rs) verifies typed, source-local exactness only under explicitly attested binary collation and no character padding, plus fixed-width CHAR negative controls. Nonportable ILIKE, SIMILAR TO, regex and locale collation variants remain unproved even when unrelated binary assumptions are declared. No external vendor/session execution is attested.
- [Independent review handoff](task-192-independent-review-handoff.md) links every original variant family to its physical and typed proof, the still-unapproved disposition and the responsible downstream reviewers. The 559-cell source of truth and individual `independently_reviewed=false` flags are unchanged.
- [Production sql-tdg migration proposal](https://github.com/phdah/sql-tdg/pull/50) is a separate draft from the earlier temporary compatibility harness [#49](https://github.com/phdah/sql-tdg/pull/49). The production proposal still uses an unreleased commit pin and must not merge until an independent migration review and a matching released SemVer version exist.

This is real additional evidence for source-local claims and conservative negative cases. It does **not** satisfy the independent 559-cell semantic review, mandatory unapproved whole-query residual dispositions, TASK-179 vendor laws, TASK-191 STRUCT/parser adjudication or any TASK-88/TASK-91 release gate. All five bounded acceptance criteria remain unchecked.

### Sixth pass: original S013 finite semijoin and manually stated 43-case shapes

- `tests/task_192_correlated_exists.rs` uses the unchanged S013 SQL, separate physical occurrence IDs and signed/NULL SQL `id` values, and an independently specified complete DuckDB population with repeated outer values, duplicated matching inner rows, an unmatched outer and a NULL key. It verifies the full original selected terminal and all TRUE/FALSE/UNKNOWN/NOT TRUE outer occurrences; a matched inner duplicate must **not** multiply the outer terminal.
- `src/physical_realization/exists_bags.rs` now accepts an explicitly schema-backed, unique text `__physical_row_id` independent of the source SQL `id` value. This is opt-in, backward compatible and only used as physical row identity. The original S013 can now be checked with `physical_finite_exists_bag_plan` across all 13 public parser dialects; selected/rejected row IDs, every inner match and absence, the complete terminal bag, and the closed-world output domain are compared with independently fixed expectations. Repeated physical identities must be Impossible. **A finite exact output domain for this particular fully supplied bag is not a universal source-independent projected-id range.**
- `docs/task-192-original-query-shapes.json` and `tests/task_192_independent_query_shapes.rs` independently state 43 original SQL outer-source, output and canonical predicate-shape facts, including the two S179 parser rejects. This protects against matching only the first analyzer-produced full document, though it is **not** a complete manually authored 43-document semantic golden or a distinct reviewer signoff.

The ledger is an exhaustive expected **public contract**, not a signed release certification: residual computed/temporal and vendor-dependent cases still require independent adjudication against their original bounded acceptance obligations. External vendor laws, final downstream migration and independent reviewer signoff retain their explicit owners. **No task Done or parent release approval is implied.**
