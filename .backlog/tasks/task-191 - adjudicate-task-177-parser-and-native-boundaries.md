---
id: TASK-191
title: Adjudicate TASK-177 parser and native boundaries
status: Done
assignee: []
created_date: '2026-11-09'
updated_date: '2026-11-18'
labels: []
milestone: m-3
dependencies:
  - TASK-179
references:
  - TASK-88
  - TASK-177
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-88 child 4/5, extracted from the original broad TASK-177 acceptance criteria by maintainer-approved scope split.** Complete the **finite outstanding exception-disposition work**, not the already-frozen parser inventory and not TASK-178's shared-AST semantic equivalence or TASK-179's vendor/session laws. Use its own PR against `main` after TASK-179 merges. Original TASK-88 AC #1..#8 and the protocol-wide outcome-first Definition of Done remain mandatory.
<!-- SECTION:DESCRIPTION:END -->

## Bounded scope and source of truth

- **Stable fixture IDs:** `88-C191-P`, `88-C191-N`. The 13 supported dialects are an exact parser-domain set; record explicit per-cell evidence and preserve all original fixture references.

- 55 exact parser-rejected SQL/dialect cells in `docs/task-177-parser-boundaries.json`.
- 116 exact public-analyzer Unsupported SQL/dialect cells in `docs/task-177-analysis-boundaries.json`.
- DuckDB-original boundary checks must preserve physical source bags and separate native engine from parser/adapter claims.
- 23 original SQL samples without successful fixed native terminal/state proof in `docs/task-177-remaining-original-cases.json`. Their current breakdown is **15 conditional/rejected syntax, five nondeterministic samples, one nonterminating recursion, one RANDOM(), and one optimizer-sensitive EXPLAIN**.
- Join on the stable `88-C177-Snnn` sample IDs and dialect identities. A boundary may appear in multiple registers. Do not double-count it or infer certified semantics from parser success, companion SQL or a different vendor.
- Independently reviewed canonical divergence for 23 cells is owned by **TASK-178**; vendor/session/transaction laws by **TASK-179**; whole-matrix typed domains, complete positive/negative direction and SQL/dbt/ODCS release parity by **TASK-180**. This ticket owns the residual parser, Unsupported and original-execution boundary dispositions and hands them to those owners.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Freeze an executable, versioned TASK-191 disposition ledger joining **exactly 55 parser rejections, 116 Unsupported cells and 23 native-unproven original samples** to their TASK-66 IDs, dialects, source fixtures, physical evidence, dependent semantic owner and required release decision. Detect missing/duplicate rows and stale TASK-177 snapshots.
- [x] #2 For each rejected parser cell, either prove a meaning-preserving alternate spelling with equivalent canonical typed claims and independent native evidence, or retain a named parser/engine limitation with assumptions and an explicit reviewed conditional/Impossible/Residual decision. Never mark a rejected original SQL spelling as natively executed merely because the alternative runs.
- [x] #3 For every public-analyzer Unsupported cell, implement a sound, strongest-safe typed outcome with regression evidence **or** document a verified source-bound unsupported/residual classification, its precise diagnostics, affected capability, owner and reviewer disposition. An unimplemented mandatory advertised feature cannot be reclassified as an approved Residual to make CI green.
- [x] #4 Adjudicate the 23 native-unproven original samples with independently specified fixed source/output/poststate and selected/NOT TRUE evidence **where execution is meaningful**; for native errors, nontermination, RANDOM/sampling, external I/O and EXPLAIN retain executable negative/bounded or explicit engine/session conditional proof. Never manufacture deterministic output, skip failed samples or promote a companion to an original proof.
- [x] #5 Reconcile resolved/conditional rows into TASK-178/TASK-179/TASK-180 evidence and `docs/coverage-manifest.json`; independently review each bounded TASK-191 decision and pass final-head fmt, Clippy, default/no-default Rust tests, docs, dbt E2E and pinned sql-tdg consumer checks. Keep TASK-88 and release approval blocked on outstanding mandatory global semantic proofs.
<!-- AC:END -->

## Next

After this child is **Done and merged**, [TASK-180](task-180%20-%20independently-certify-complete-dialect-matrix-and-gate-ci.md) performs the independent complete-matrix and TASK-88 parent signoff. After TASK-178 PR #197 merges, TASK-192 is the immediate next task. TASK-191 still follows TASK-179 after the extracted TASK-192..197 sequence; the parser/native acceptance obligations are unchanged.

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Created from TASK-177 scope split on 2026-11-09. Baseline handoff: `docs/task-177-scope-split.md`, `docs/task-177-acceptance-audit.md`, `docs/task-177-physical-proof-ledger.json`. No implementation or acceptance signoff for this new ticket yet.
<!-- SECTION:NOTES:END -->

## TASK-178 bounded-scope handoff (2026-11-10)

The original TASK-178 semantic family proofs are now TASK-192..196, with cross-adapter acceptance TASK-197 and TASK-180 final signoff. TASK-191 still owns exactly the original 55 parser rejects, 116 Unsupported cells, and 23 original native nonterminals without double counting. Do not close mandatory parser/Unsupported claims using only TASK-178's observational inventory. See [scope-split ownership](../../docs/task-178-scope-split.json).


## TASK-192 pending original parser/native handoff (not an approval)

- Original S179 `CAST(ROW(1,'a') AS STRUCT(a INT,b VARCHAR))` succeeds natively in DuckDB with a structured scalar but remains a typed canonical Unknown/Unsupported output in the current portable protocol. BigQuery and generic parsers reject that **original spelling**; the DuckDB historical drift is explicitly retained/repaired. TASK-191 owns the later parser/STRUCT disposition rather than treating the native DuckDB scalar as a universal supported output.
- Original S033 signed fixed-width overflow is a native **error** (not a successful scalar), S042 NULLS LAST ordered bag is independently captured without inventing tie ordering, and S027/S032 engine-specific arithmetic width differences are recorded. TASK-191 retains its own independently reviewed boundary ledger and separate Done gate.

## TASK-191 initial implementation (2026-11-18)

- The task-owned [v1 immutable acceptance freeze](../../docs/task-191-acceptance-freeze.json) joins exactly 55 rejected parser cells, 116 canonical Unsupported cells and 23 original native nonterminal cases to their original source SQL IDs, semantic owners and conservative reviewed-proof obligations. The 13 source dialect identities and the exact original SQL spelling remain authoritative in TASK-177.
- [Public API and DuckDB acceptance tests](../../tests/task_191_boundary_disposition.rs) re-evaluate the original error/Unsupported boundaries and two physical companion oracles with complete nullable, duplicate and stable row-identity source bags. Original LEFT SEMI/ANTI DuckDB rejection is checked explicitly. No external native vendor/runtime certification or alternate-original equivalence is claimed.
- [Acceptance audit](../../docs/task-191-acceptance-audit.md) itemizes every remaining signoff gate. **All five ACs remain unchecked** until independent per-cell reviews, full CI, external limitations and cross-adapter parity have actually been validated. TASK-88, TASK-180 and release TASK-91 remain blocked.

## TASK-191 complete exception review candidate (2026-11-18)

- [v2 independent source-bound disposition](../../docs/task-191-reviewed-dispositions-v2.json) audits the original 55 parser, 116 analyzer and 23 native exception entries without weakening any release-blocking mandatory feature.
- [v3 TASK-178/179/197/180 handoff](../../docs/task-191-downstream-handoff-v3.json) retains 194 disjoint review obligations, source-fact adapter authority, exact SQL and native-vendor limitations.
- Added task-local original [native execution](../../tests/task_191_native_original_adjudication.rs), [public diagnostics](../../tests/task_191_independent_review.rs), and [downstream parity reconciliation](../../tests/task_191_downstream_handoff.rs) tests.
- Acceptance signoff remains pending final-head verification, independent review and pinned consumer. TASK-88/TASK-180/TASK-91 stay separately blocked.

## Final bounded TASK-191 acceptance signoff (2026-11-18)

**All five TASK-191 child acceptance criteria are verified.** The immutable initial [v1 freeze](../../docs/task-191-acceptance-freeze.json) is reconciled by the [194-decision independent source-bound review](../../docs/task-191-reviewed-dispositions-v2.json), [exact downstream ownership handoff](../../docs/task-191-downstream-handoff-v3.json), and [final review record](../../docs/task-191-independent-review-v4.json).

- AC1: exactly 55 parser rejection cells, 116 public Unsupported cells and 23 missing original native terminal cases, with unchanged TASK-66 source IDs, all 13 dialect identities where applicable, exact original SQL and immutable TASK-177 snapshots.
- AC2: every parser rejection receives reviewed exact-original-syntax Impossible and vendor-runtime Residual evidence with named limitation and blocking feature owner; no alternate spelling is mistaken for successful original execution.
- AC3: all 116 unsupported analyzer cases retain exact category/code/message/severity/area and typed Unknown, with advertised mandatory feature implementation still a **release blocker**, not an approved deferral.
- AC4: all 23 original native cases are independently replayed or safely bounded: 15 errors/invalid original SQL with unchanged full physical source row identities, five stochastic source subbags, nonterminating recursive companion only, original RANDOM value bounds, and EXPLAIN's engine-sensitive variable plan. No fixed original native terminal is invented.
- AC5: independently reviewed 194 original decisions, TASK-178/TASK-179/TASK-197 source evidence reconciled for TASK-180, 9/9 protocol CI on `44f8f497af57e3f26e82ec5e6c2002111730f24b` ([run](https://github.com/phdah/sql-semantic-protocol/actions/runs/44405445326)) and 5/5 exact-pin temporary [sql-tdg CI](https://github.com/phdah/sql-tdg/actions/runs/44405560913) ([PR #57](https://github.com/phdah/sql-tdg/pull/57)). The signoff task-state commit must itself pass the full protocol CI before merging.

**Scope restriction:** TASK-191 Done means its conservative exception adjudication is complete, **not** that unsupported mandatory SQL semantics, unexecuted vendor engines, parent TASK-88, TASK-180 or release TASK-91 are approved. Next TASK-180 remains dependency-blocked until PR #205 is merged to main.
