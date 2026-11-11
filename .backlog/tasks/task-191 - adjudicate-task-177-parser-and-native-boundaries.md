---
id: TASK-191
title: Adjudicate TASK-177 parser and native boundaries
status: To Do
assignee: []
created_date: '2026-11-09'
updated_date: '2026-11-09'
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
- [ ] #1 Freeze an executable, versioned TASK-191 disposition ledger joining **exactly 55 parser rejections, 116 Unsupported cells and 23 native-unproven original samples** to their TASK-66 IDs, dialects, source fixtures, physical evidence, dependent semantic owner and required release decision. Detect missing/duplicate rows and stale TASK-177 snapshots.
- [ ] #2 For each rejected parser cell, either prove a meaning-preserving alternate spelling with equivalent canonical typed claims and independent native evidence, or retain a named parser/engine limitation with assumptions and an explicit reviewed conditional/Impossible/Residual decision. Never mark a rejected original SQL spelling as natively executed merely because the alternative runs.
- [ ] #3 For every public-analyzer Unsupported cell, implement a sound, strongest-safe typed outcome with regression evidence **or** document a verified source-bound unsupported/residual classification, its precise diagnostics, affected capability, owner and reviewer disposition. An unimplemented mandatory advertised feature cannot be reclassified as an approved Residual to make CI green.
- [ ] #4 Adjudicate the 23 native-unproven original samples with independently specified fixed source/output/poststate and selected/NOT TRUE evidence **where execution is meaningful**; for native errors, nontermination, RANDOM/sampling, external I/O and EXPLAIN retain executable negative/bounded or explicit engine/session conditional proof. Never manufacture deterministic output, skip failed samples or promote a companion to an original proof.
- [ ] #5 Reconcile resolved/conditional rows into TASK-178/TASK-179/TASK-180 evidence and `docs/coverage-manifest.json`; independently review each bounded TASK-191 decision and pass final-head fmt, Clippy, default/no-default Rust tests, docs, dbt E2E and pinned sql-tdg consumer checks. Keep TASK-88 and release approval blocked on outstanding mandatory global semantic proofs.
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
