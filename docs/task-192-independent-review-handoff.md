# TASK-192 independent semantic review handoff

**Status: awaiting independent reviewer and mandatory residual dispositions.**
This is a review aid, not a signoff. Neither parser acceptance nor successful
DuckDB sample execution is evidence that all 13 native vendors implement the
same runtime behavior.

## Fixed acceptance boundaries

- 43 unchanged original SQL variants and 13 public parser dialects: 559
  named cells in `docs/task-192-acceptance-freeze.json`.
- 559 explicit expected *public* analyzer classifications in
  `docs/task-192-semantic-cell-ledger.json`. The ledger remains unreviewed.
- 40 complete native DuckDB TRUE/FALSE/UNKNOWN/NOT TRUE predicates, plus
  original S033 overflow error, S042 NULL ordering and S179 STRUCT scalar.
- Ten independent strongest scalar set/range projections; unrelated
  `id` projections must not acquire a finite domain.
- No non-DuckDB vendor execution is claimed, and no missing core feature
  may be designated an approved residual to waive acceptance.

## Case-by-case review route

| Original case IDs | Independently fixed evidence | Typed/operator proof status | Required review decision |
| --- | --- | --- | --- |
| S001-S009, S012 | TASK-177 numeric native full bags, exact signed bounds and inclusive endpoints, TASK-192 559-cell emitted contract | Exact numeric scalar output where independently constrained; S012 retains full nullable OR witness | Independently confirm bound signs, NULL exclusion and 13-dialect canonical equality |
| S010, S011 | TASK-177 original 3VL partitions; `task_192_nullable_truth.rs` proves physical TRUE/NOT TRUE across 13 dialects; `task_192_not_equality.rs` independently proves S011 signed <=1 and NULL exclusion and rejects FLOAT/untyped complements | S010 same-row OR and S011 signed NOT are whole-query Exact **only** under explicit signed-integer catalog evidence; S010 retains the correlated Boolean tree without inventing independent scalar ranges; S011 has signed `amount <= 1` and no projected `id` bound. Both untyped originals remain Residual; FLOAT remains fail-closed | Independently audit both typed-only promotions, 3VL source rows, one-source correlation, NULLs, NaN/floating negatives and direct/dbt/ODCS parity; no finite `id` domain |
| S013 | `tests/task_192_correlated_exists.rs`: original SQL, complete outer/inner DuckDB bags, 13-parser exact physical semijoin, plus negative test separating *missing* inner bag from explicitly complete *empty* inner bag | Bounded source-bag implementation is complete: exact selected/rejected outer identities, all inner partners, NULL and duplicates, and full terminal; universal open-world scalar domain stays Residual necessarily | Independently approve the precise closed-world preconditions and distinct TASK-193 relational ownership, without interpreting absent inner data as no matches |
| S014-S019 | Original native three-valued partitions and 13-dialect public contract | S016 IS TRUE, S017 IS FALSE and S018 IS UNKNOWN preserve NULL truth; typed S016/S017 negative FLOAT controls | Independently review NULL truth, NULL-containing IN and source type assumptions |
| S020, S022, S025 | Native pattern partitions, `task_192_string_witness.rs` | Exact source-local safe prefix only for typed VARCHAR and both declared binary-collation/no-padding laws; missing law and fixed CHAR remain Residual | Review the caller attestation and reject inferred nonbinary/CHAR behavior |
| S021, S023, S024, S026 | Native ILIKE/SIMILAR/regex/collation partitions, conservative 13-dialect negative tests | Unproved source-local vendor/session semantics, even after unrelated binary attestations | TASK-179 vendor/session disposition; no blanket portable Exact classification |
| S027-S032 | Complete original DuckDB partitions, plus `task_192_signed_computed_bags.rs` on 13 parsers: full schema-validated source-bag TRUE/NOT TRUE/UNKNOWN and duplicate terminal with checked signed widths | S028-S031 operator-local TRUE/NOT TRUE witnesses already Exact. **S027/S032 now have exact closed-world signed 16/32/64-bit physical proofs when every arithmetic result fits its declared source width**; signed extrema safely return Residual without inferring native widening or errors | Review source-width guards, terminal set/bounds, NULL/duplicates, safe vs overflow examples, and TASK-179 vendor arithmetic handoff; never infer a global finite projected id domain |
| S033 | DuckDB fixed-width overflow error | No successful native terminal and no invented scalar output domain | TASK-191 parser/unsupported, TASK-179 vendor overflow law |
| S034-S036 | TASK-192 native temporal partitions and original emitted residuals | Computed temporal and implicit conversion semantics remain unproved | TASK-179 timezone, DST, result-type and conversion law |
| S037, S039, S040 | Original native timezone, padded-string and NaN partitions with separately bounded source/session laws | S037/S039 have Conditional untyped public outcomes; S039 has typed source-local Exact only after binary/no-padding attestation. S040 is now an explicit Residual for unattested text-to-NaN coercion, even with unrelated assumptions declared | TASK-179 independent vendor/session and implicit numeric-cast law review, with no fabricated native attestation |
| S038, S041 | Native collation and signed-zero partitions | Vendor-specific/nonportable semantics remain unapproved | TASK-179 collation/NaN/signed zero disposition |
| S042 | Independent complete ordered NULL/tie bag | Original native order is witnessed without inventing tie identities | TASK-191 parser/order portability, TASK-179 NULL-order session law |
| S179 | Native STRUCT(a=1,b='a') scalar, two named parser rejections, repaired DuckDB structural drift | STRUCT typed portable cast outcome remains Unknown, not a fabricated primitive | TASK-191 STRUCT/Unsupported disposition |

All case IDs above are the `88-C177-S...` suffixes of the original
freeze. The task's source ownership and parser observations remain in the
machine-readable ledger; this table is a human review route, not a replacement.

### Original S040 coercion boundary, pending TASK-179

Untyped `amount = 'NaN'` cannot be certified using collation/padding or
`no_nan`/`signed_zero_equivalent` flags: it is the implicit string-to-float
coercion itself that remains unproved. The original 13 S040 expected cells now
name `comparison_semantics` as a Residual instead of Conditional. Review
`tests/task_192_nan_text_cast.rs`, the original DuckDB terminal and TASK-179's
unapproved vendor law before any stronger classification.

## Explicit 559-cell reviewer assignment queue

[Versioned reviewer queue](task-192-review-queue.json) lists exactly 43 original
cases and 559 parser cells, with normalized links to the original SQL, current
expected typed policies, separately authored SQL source/predicate shapes and
native source/terminal evidence. The reviewer queue also links the
[43 complete canonical candidate documents](task-192-proposed-full-documents.json),
now checked against all 557 accepted parser cells by
[the complete-document regression test](../tests/task_192_full_document_goldens.rs).
Those byte-complete candidates **originated from the analyzer** and are
**not independently authored or approved semantic goldens**. Every reviewer
identity and decision remains **pending**. The [completeness
test](../tests/task_192_review_queue.rs) prevents dropped/duplicated cells or
accidental claims of approval.

The independent reviewer must author/validate the 43 full-document goldens
without copying analyzer output as the expected answer, then record a real
review identity, immutable reviewed commit, exact per-cell decision and evidence
references. A reviewer may group identical dialect proofs but must account for
every original 559-cell ID, including both S179 parser rejections. The author
cannot sign this queue or promote TASK-192 from this generated worksheet alone.

### Three-case bounded semantic disposition for independent signoff

The machine-readable [S013/S027/S032 semantic closure matrix](task-192-bounded-semantic-closures.json)
records actual positive and negative implementation scope. These are real
portable conditional physical source-bag proofs, not blanket open-world
exactness. A missing inner population or overflowing signed source is still
an explicit and necessary Residual. The bounded source proof implementation
is complete; distinct independent semantic review must judge whether these
preconditions meet TASK-192's required strongest sound outcomes and approve
or request changes. TASK-179 owns vendor-specific overflow and TASK-193
owns broader relational generation. No unsigned owner decision is being
re-labelled as approval.

## Required signoff steps

1. Independently inspect **each of the 559 named public parser cells** against
   the frozen original SQL, independent source/terminal oracle, typed output
   domain and exactness/conditional/residual reason. Record reviewer, reviewed
   commit, individual disposition and supporting oracle reference. The
   author-written ledger is not independent approval.
2. Separate symbolic parser/typed truth claims from DuckDB's actual complete
   physical bag. Review both TRUE and NOT TRUE, all NULL and duplicate
   occurrences, and source vs projected output domains.
3. For every residual that is mandatory core behavior, obtain a sound
   constructive realization or an independently approved, explicitly scoped
   disposition from its actual owner. Do not mark an open limitation "approved"
   solely to make the count pass.
4. Confirm TASK-179 vendor/session and TASK-191 parser/STRUCT boundaries
   remain unapproved where evidence is missing. Hand this exact bounded
   evidence to TASK-197 cross-adapter and TASK-180 global matrix reviewers.
   Their *own* release-wide gates are separate from the TASK-192 child.
5. Re-run protocol exact-head fmt, strict Clippy, default/no-default tests,
   docs, dependencies, dbt Core E2E, active schema/goldens and pinned
   `sql-tdg` integration. The consumer migration must be independently
   reviewed and pinned to the eventual released protocol, not an unreleased
   temporary Git SHA.
6. Only after all five TASK-192 ACs have individually verifiable evidence,
   all mandatory cells are reviewed, and required CI is green may the child
   task become Done. TASK-88/TASK-91 remain separate, unsigned release gates.

## Reproducible evidence index

- `docs/task-192-acceptance-freeze.json`
- `docs/task-192-semantic-cell-ledger.json`
- `docs/task-192-proposed-full-documents.json`
- `tests/task_192_full_document_goldens.rs`
- `docs/task-192-native-partitions.json`
- `docs/task-177-predicate-oracles.json`
- `tests/task_192_semantic_contract.rs`
- `tests/task_192_typed_soundness.rs`
- `tests/task_192_native_partitions.rs`
- `tests/task_192_nullable_truth.rs`
- `tests/task_192_computed_witness.rs`
- `tests/task_192_string_witness.rs`
- `tests/task_192_adapter_parity.rs`
- `tests/task_192_projection_counterexamples.rs`
- [Downstream production migration draft](https://github.com/phdah/sql-tdg/pull/50)
- [Temporary pin-only compatibility harness](https://github.com/phdah/sql-tdg/pull/49)

**This document intentionally contains no signed-off reviewer identity.**
