# TASK-177 scope split and acceptance handoff (2026-11-09)

**Maintainer decision:** TASK-177's original five acceptance criteria combined inventory freezing with exhaustive semantic and release signoff. The frozen inventory work is complete and may be marked **Done as a bounded child**. The original broad semantic requirements are **transferred, not waived**. TASK-88 AC #1..#8, TASK-91 release and all inherited strongest outcome-domain requirements remain in force.

**Delivery:** [PR #196](https://github.com/phdah/sql-semantic-protocol/pull/196); Backlog [TASK-177](../.backlog/tasks/task-177%20-%20freeze-task-66-feature-by-13-dialect-parser-coverage.md). A Done marker in an **unmerged** PR does not unlock TASK-178; its PR must merge into `main` first.

## Completed, finite TASK-177 acceptance

1. Source-controlled mapping of **251 TASK-66 original variants x 13 dialects = 3,263 cells**, preserving source fixture IDs and the distinction between observed parser results and old fixture provenance.
2. CI-frozen public parse/analyze/canonical statuses: 3,208 parser accepted, 55 rejected; 3,092 protocol documents, 116 Unsupported; 3,069 structurally identical documents and 23 divergent document cells. The registers name every boundary, affected case, dialect, owner, and first diff path.
3. Case-level first-party evidence ledger: 228 distinct native original SQL output/state cases, 23 remaining without a successful fixed native terminal, ten **independent typed-domain** SQL/dbt/ODCS facts and two separate P/N physical goldens. Exposed native rejections, stochastic/recursive/EXPLAIN limitations and alternate spellings are **not promoted to success**.
4. Full pre-split protocol [9/9 CI](https://github.com/phdah/sql-semantic-protocol/actions/runs/42697295355) and exact-candidate-pinned sql-tdg [5/5 CI](https://github.com/phdah/sql-tdg/actions/runs/42697390328). PR #196's final scope-split commit must also pass protocol/consumer CI before merge.
5. All outstanding requirements are mapped to separate blocking tasks with an explicit dependency order.

The original `docs/task-177-acceptance-freeze.json` is retained **unaltered as the historical pre-split 3,263-cell baseline**. Its `completion.child_done=false`, individual uncertified-cell markers and related tests record **the original full semantic certification**, not the new scoped Backlog task completion. Do not treat the old completion flag, or current TASK-177 Done, as a statement of global semantic signoff.

## Original TASK-177 AC transfer, no silent deferrals

| Historical AC | Completed subset | Remaining mandatory owner |
| --- | --- | --- |
| #1 Complete finite acceptance matrix | All named 251 x 13 SQL parser/analyzer status observations and fixture provenance are frozen | TASK-191 boundary dispositions; TASK-178 strongest typed shared AST; TASK-180 all exact/correlation/terminal cell certifications |
| #2 Sound composed semantics | Structural canonical comparison; ten typed portable outcomes; existing native fixtures | TASK-178 full typed, multiplicity and composition equivalence; TASK-180 independent complete-output and source/adapter proof |
| #3 Positive/rejected native differentials | 228 original native whole terminal/state oracles; P/N goldens and separate native negatives | TASK-191 23 incomplete original native outcomes; TASK-178 and TASK-180 complete positive/rejected/Impossible/Residual and source correlation |
| #4 13 dialects and adapter/vendor parity | All 3,263 public parser/analyzer comparisons; exact exception registries and ten source-parity facts | TASK-178 23 canonical drifts; TASK-179 vendor/session/collation/transaction laws; TASK-191 55 parser/116 Unsupported boundaries; TASK-180 complete representable SQL/dbt/ODCS and global matrix |
| #5 Final CI and independent signoff | Protocol and exact-head consumer CI passed at documented candidate; source-controlled evidence index | TASK-191 own bounded independent review; TASK-180 independent per-cell & mapped TASK-88 AC review; TASK-91 separate release gate |

## New finite exception child

[TASK-191](../.backlog/tasks/task-191%20-%20adjudicate-task-177-parser-and-native-boundaries.md) explicitly owns the **55 exact parser rejected dialect cells, 116 analyzer Unsupported cells and 23 unproven original native cases** with a frozen fail-closed disposition matrix. This ticket must not claim that a compiler rejection is impossibility, an arbitrary function is supported, an unbounded query terminates, or one DuckDB execution certifies another vendor.

**Dependency order:** TASK-177 Done **and merged** -> TASK-178 -> TASK-179 -> **TASK-191** -> TASK-180 independent whole-matrix certification -> TASK-88 parent review -> TASK-91 release gate. No dependency cycle; release-approved remains false.

## Audit boundary

`docs/task-177-acceptance-audit.md` is the historical evidence audit. Its original broad AC table is a **transferred requirement ledger**, not an unchecked TASK-177 scoped checklist. `docs/coverage-manifest.json` records scoped child completion separately from `independent_review_verified=false` and the other still-false global semantic/vendor/release booleans. This split changes **task scope and routing only**, not public protocol semantics or proof strength.
