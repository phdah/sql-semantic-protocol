# TASK-179 acceptance audit

Status: **In Progress, exact-head CI and final task signoff pending**. Parent TASK-88 and release TASK-91 are not approved.

## Versioned evidence

- Preimplementation 14-case / 182-parser-cell frozen native controls: `docs/task-179-acceptance-freeze.json`. This initial freeze preceded the first implementation slice.
- Explicitly reviewed post-freeze extension of all six finite law families: 35 original immutable SQL shapes and 2 task-owned identifier-case probes, **37 cases x 13 dialects = 481 cells**, in `docs/task-179-variant-matrix-v2.json`.
- `docs/task-179-source-and-outcome-evidence-v4.json` freezes each original source SQL/setup, complete typed source bag, nullable/bounded output-domain claims and physical TRUE/NOT TRUE partitions or complete terminal/error/tie Residual. It transparently describes the scope amendment rather than pretending all 37 cases were frozen before the initial slice.
- `docs/task-179-independent-481-cell-signoff-v5.json` records each named parser, output, source, adapter and vendor/session verdict. `docs/task-179-independent-review-v6.json` is the second-pass standards/spec review.
- Independent executable checks: `tests/task_179_full_original_native_replay.rs` replays all 37 fixed DuckDB source and terminal/error/tie cases; `tests/task_179_original_truth_replay.rs` checks disjoint and complete TRUE/NOT TRUE source occurrence partitions; `tests/task_179_independent_481_review.rs` checks all 481 review decisions against immutable owners, original SQL and exact typed/physical evidence.
- Previously reviewed TASK-192, TASK-195, TASK-196, TASK-163 typed and physical evidence remains the owner of original portable SQL facts. TASK-197 direct SQL, dbt compiled manifest/catalog and ODCS adapter tests remain authoritative for the 31 representable original cases. Four transaction scripts and two identifier probes cannot inherit fake adapter runtime attestation.

## Conditional vendor-law disposition

A successful sqlparser dialect is **not** a vendor engine. Non-DuckDB vendor semantics remain explicitly **Residual** with named engine/version, session, native physical source and terminal/error evidence required. This is an approved conservative classification candidate, **not** an external native-engine success. No vendor execution is manufactured.

The original cell statuses are 416 source-analyzed, 35 unsupported, 4 rejected and 26 task-owned identifier probes that now have executable parser/canonical tests. Bounded native DuckDB outcomes are attested only under their fixed session/source setup. S162's unspecified peer ordering and S033's overflow never receive a fabricated terminal.

## Acceptance signoff checklist

| AC | Evidence state | Remaining |
| --- | --- | --- |
| #1 | Complete 37-variant versioned matrix, 481 per-cell evidence decisions, exact source and terminal/error/tie snapshots, typed NULL/bound policies and truthful reviewed amendment | Final-head verification of all linked tests and second-pass review |
| #2 | Existing strongest-sound portable typed source domains carried unchanged from TASK-192/195/196, no invented independent witness conjunction or vendor exactness | Final-head regression suite |
| #3 | Public-API native DuckDB source/terminal and TRUE/NOT TRUE counterexamples, including original transactions, duplicates, NULL, exceptional scalar and tied order | Final-head regression suite |
| #4 | All 481 parser/equivalence dispositions and 31 authoritative direct SQL/dbt/ODCS source fact adapter cases; unrepresentable metadata and external native vendor obligations are named Residual | Final per-cell review reconciliation |
| #5 | Requires final-head fmt, strict all-target Clippy, full/no-default Rust tests/docs, dbt E2E, active golden/schema and 5/5 exact-pin sql-tdg consumer checks | Exact-head CI and pinned consumer CI, then independent final signoff |

**Do not mark the task Done until the final exact-head 9/9 protocol CI and 5/5 pinned consumer CI pass.** TASK-191, TASK-180, TASK-88 and TASK-91 remain separate next-stage obligations.
