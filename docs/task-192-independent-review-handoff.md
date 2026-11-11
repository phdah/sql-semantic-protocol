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
| S010, S011 | TASK-177 original native 3VL partitions; `task_192_nullable_truth.rs` additionally checks public `physical_source_plan` source-backed TRUE/NOT TRUE obligations across all 13 dialects; direct/dbt/ODCS compiled witness parity | Signed-integer typed row-local and public physical-source TRUE/NOT TRUE witnesses Exact, but whole-query independent-domain exactness remains Residual | Review Boolean same-row correlation and NOT/UNKNOWN, preserve arbitrary projected-id domain; do not mistake an individual constructible source row for full terminal-bag or universal finite-domain proof |
| S013 | `tests/task_192_correlated_exists.rs`: original SQL, independent complete outer/inner DuckDB bags and 13-parser closed-world physical semijoin proof | Exact *finite supplied-source* selected/rejected outer identities, all inner partners, and terminal bags with numeric/NULL SQL `id` distinct from `__physical_row_id`; whole-query universal scalar contract remains Residual | Independently audit source completeness, duplicate/NULL occurrence law, proven finite-bag vs universal outcome boundary, and any downstream TASK-193 relational scope |
| S014-S019 | Original native three-valued partitions and 13-dialect public contract | S016 IS TRUE, S017 IS FALSE and S018 IS UNKNOWN preserve NULL truth; typed S016/S017 negative FLOAT controls | Independently review NULL truth, NULL-containing IN and source type assumptions |
| S020, S022, S025 | Native pattern partitions, `task_192_string_witness.rs` | Exact source-local safe prefix only for typed VARCHAR and both declared binary-collation/no-padding laws; missing law and fixed CHAR remain Residual | Review the caller attestation and reject inferred nonbinary/CHAR behavior |
| S021, S023, S024, S026 | Native ILIKE/SIMILAR/regex/collation partitions, conservative 13-dialect negative tests | Unproved source-local vendor/session semantics, even after unrelated binary attestations | TASK-179 vendor/session disposition; no blanket portable Exact classification |
| S027-S032 | Original complete computed DuckDB partitions in `task_192_computed_witness.rs` | S028-S031 signed-integer typed TRUE/NOT TRUE witnesses Exact; S027 addition and S032 ABS remain unproved without overflow-range laws | Verify original expression/NULL direction, overflow boundaries and strongest *projected id* domain |
| S033 | DuckDB fixed-width overflow error | No successful native terminal and no invented scalar output domain | TASK-191 parser/unsupported, TASK-179 vendor overflow law |
| S034-S036 | TASK-192 native temporal partitions and original emitted residuals | Computed temporal and implicit conversion semantics remain unproved | TASK-179 timezone, DST, result-type and conversion law |
| S037, S039, S040 | Original native timezone, padded-string and NaN partitions; conditional public contract | Conditional owner-named laws; S039 has a typed source-local Exact witness **only after binary/no-padding attestation** | TASK-179 independent vendor/session law review and validation that source assumptions actually hold |
| S038, S041 | Native collation and signed-zero partitions | Vendor-specific/nonportable semantics remain unapproved | TASK-179 collation/NaN/signed zero disposition |
| S042 | Independent complete ordered NULL/tie bag | Original native order is witnessed without inventing tie identities | TASK-191 parser/order portability, TASK-179 NULL-order session law |
| S179 | Native STRUCT(a=1,b='a') scalar, two named parser rejections, repaired DuckDB structural drift | STRUCT typed portable cast outcome remains Unknown, not a fabricated primitive | TASK-191 STRUCT/Unsupported disposition |

All case IDs above are the `88-C177-S...` suffixes of the original
freeze. The task's source ownership and parser observations remain in the
machine-readable ledger; this table is a human review route, not a replacement.

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
