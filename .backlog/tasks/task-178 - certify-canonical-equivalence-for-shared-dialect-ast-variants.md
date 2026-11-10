---
id: TASK-178
title: Establish portable canonical-equivalence baseline for shared AST variants
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-10'
labels: []
milestone: m-3
dependencies:
  - TASK-177
references:
  - TASK-88
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-88 original child 2/5, bounded baseline completed in PR #197 by maintainer-approved scope split (2026-11-10).** This ticket delivers the 13-parser portable canonical-equivalence and explicit uncertainty/negative evidence foundation, NOT whole-matrix semantic certification. The original five acceptance criteria are preserved verbatim and allocated to TASK-192..197 plus existing TASK-179, TASK-191 and TASK-180 in [the split decision](../../docs/task-178-scope-split.md) and [machine-readable ownership](../../docs/task-178-scope-split.json). TASK-88 and TASK-91 remain blocked. No child depends on TASK-88.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope (approved amendment, 2026-11-10)

- **Stable baseline IDs:** `88-C178-P`, `88-C178-N` and the original `88-C177-Snnn` SQL.
- **Delivered here:** 7-case positive/negative DuckDB starter goldens; all 163 named original SQL cases x 13 parser dialects classified without omissions; meaning-preserving canonical variants and fail-closed unknowns; 78 original adapter/domain parity variants and 53 independently pinned DuckDB terminal/domain-containment checks.
- **Explicitly not delivered here:** strongest exact typed and independent complete source/terminal proof for all 2,119 cells, across every original family, nor vendor/native execution attestation, unimplemented mandatory support or independent matrix reviewer approval.
- **Original parent AC:** #2, #4, #5, #7, #8, #6 remains mandatory through TASK-192..197 and TASK-179/191/180. The retrospective seven-case freeze deviation remains recorded.
- **Evidence gates:** Register the bounded TASK-178 completion and task ownership in `docs/coverage-manifest.json`. The existing `88-C178-P`/`88-C178-N` fixture IDs prove only selected DuckDB cases; every unproved `Residual` or unexecuted external engine claim stays explicit. All 13 project parser dialects remain represented.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Preserve stable original SQL/fixture IDs and frozen TASK-177 historical classifications. Commit a versioned seven-case bounded acceptance freeze and the complete 163 x 13 *observed* parser/canonical ledger with every unproved typed/physical/vendor cell explicitly marked non-certified.
- [x] #2 Implement and verify meaning-preserving portable JOIN/set/predicate and TOP/FETCH/LIMIT canonical spelling tests across 13 parser dialects, plus fail-closed ROLLUP/CUBE empty-source and STRUCT CAST corrections. Preserve explicit WITH TIES and MSSQL LATERAL parser meaning boundaries rather than claiming universal equivalence.
- [x] #3 Add public-API and independent DuckDB source/terminal positive/negative NULL/duplicate baseline oracles for predicate, join, set, window, pagination, INSERT and CTAS; extend output-domain *soundness* checks to 53 original relational cases and direct/dbt/ODCS typed-domain parity to 78 original cases. Do not infer strongest possible global domains or external vendor native execution from these.
- [x] #4 Preserve a truthful 14-case original DuckDB nonterminal exception registry and 22 repaired/one retained historical canonical-drift overlay. Version the exact category-by-category handoff to TASK-192..197 and TASK-179/191/180, retaining every original requirement and TASK-88 release blocker.
- [x] #5 Verify final-head fmt, strict Clippy, full/no-default Rust tests/docs, dbt E2E and exact-head pinned sql-tdg compatibility for this bounded PR. Keep independent full-matrix reviewer signoff, global acceptance and TASK-88/TASK-91 release approval open for TASK-180 and parent gate.
<!-- AC:END -->

## Next

Next: [TASK-192](task-192%20-%20certify-predicate-and-structural-type-outcomes.md), only after PR #197 merges. The extracted queue runs through TASK-197 before existing TASK-179, TASK-191 and TASK-180. This Done status applies only to the bounded baseline, never the original globally unpartitioned TASK-178 AC or TASK-88 parent.

## Initial implementation PR (2026-11-09)

- **Versioned v1 task-local acceptance baseline:** [docs/task-178-acceptance-freeze.json](../../docs/task-178-acceptance-freeze.json), linked from the coverage manifest. This freezes seven first-party, all-13-dialect representative SQL families (predicates, joins, sets, windows, pagination, INSERT and CTAS) with explicit physical NULL/duplicate source and terminal bags, positive and rejected directions where applicable, and a conservative unsigned matrix.
- **Executable public/native tests:** `tests/task_178_portable_equivalence.rs` checks the exact original TASK-177 sample SQL against complete emitted public protocol documents, and independent DuckDB positive/negative bag oracles, including an inclusive typed predicate lower bound and source-preserving writes.
- **Still blocking:** the complete TASK-178 family/SQL synonym matrix (especially TOP, FETCH, MINUS), 23 preexisting public drift cells, strong typed outcome/oracle coverage for all mandatory cells, native engine/session evidence, direct/dbt/ODCS equivalence and independent review. No new type or semantic support is claimed for unsupported SQL or unexecuted vendors.
- **No AC signed off yet:** the finite task freeze is only a starter subset, not the full AC #1 inventory. TASK-178 remains In Progress and cannot be marked Done or unblock TASK-179 before independent final verification, successful final-head CI, and merge. TASK-88 and TASK-91 remain open.


## Expanded implementation and signoff disposition (2026-11-10)

- **Meaning-preserving dialect AST normalization:** `src/analysis/aggregation.rs` and `src/analysis/queries.rs` canonicalize dedicated/function ROLLUP and CUBE without inventing zero-output from an empty physical source. `src/analysis/expressions.rs` preserves a single explicitly unknown STRUCT CAST across dedicated/custom sqlparser target types. The historical TASK-177 snapshot is unchanged; `docs/task-178-canonical-reconciliation.json` separately records 22 expected repaired drift cells and one deliberate MSSQL LATERAL ON TRUE parser boundary.
- **Independent public-protocol equivalence:** `tests/task_178_pagination_canonical.rs` compares the entire protocol for recognized ordered TOP/FETCH FIRST ROWS ONLY/LIMIT across 13 dialects. `src/analysis/exactness.rs` emits one sound typed row-shaping residual for those variants, while unproved forms remain residual and WITH TIES stays non-equivalent to LIMIT. `docs/task-178-syntax-dialect-boundaries.json` preserves before/after classifications. MINUS/EXCEPT, JOIN spellings and set NULL/multiplicity have independent first-party regressions.
- **Native DuckDB evidence:** the 163 named frozen SQL cases include 149 previously executed complete first-party native terminals. `docs/task-178-native-boundary-adjudications.json` and `tests/task_178_native_boundary_adjudications.rs` own the remaining 14 original SQL negative or conditional cases: eight unsupported native spellings, one overflow, and five stochastic sampling cases. A rejected or random original is never called a positive native oracle.
- **Adapter evidence:** `tests/task_178_adapter_typed_parity.rs` tests direct SQL, compiled dbt and ODCS source types, NULL/duplicate/NOT TRUE physical predicate cases. `tests/task_178_adapter_family_parity.rs` adds direct SQL/dbt/ODCS output-domain parity for portable predicate, equijoin, UNION ALL, ROW_NUMBER and pagination across 13 parser dialects. dbt producer materialization is not mislabeled as physical execution. Prior TASK-177 has ten more independently expected exact predicate/domain cases, with narrower proof scope.
- **Protocol CI:** [42781880005](https://github.com/phdah/sql-semantic-protocol/actions/runs/42781880005) passed 9/9 for the canonical pagination fix; [42782568829](https://github.com/phdah/sql-semantic-protocol/actions/runs/42782568829) passed 9/9 for all fourteen native exceptions. Later changes require fresh final-head verification.
- **Pinned sql-tdg downstream CI:** a temporary non-mergeable [consumer PR #48](https://github.com/phdah/sql-tdg/pull/48) pinned protocol commit `86e9de1be053df96b96640188afc008be09162c8`, and its [CI 42784231426](https://github.com/phdah/sql-tdg/actions/runs/42784231426) passed all five jobs including dbt E2E after applying earlier validated consumer-test fixture repairs. This is **not** an exact-later-head check, and consumer changes are not intended for merge.
- **Still open, not waived:** AC #1 requires independently approved typed/source/terminal provenance across all 2,119 mandatory dialect cells and a retrospective process deviation from the initial seven-case freeze; AC #2 and #3 need complete strongest-domain/TRUE/NOT TRUE/multi-source write evidence beyond the covered oracles; AC #4 needs complete representable adapter and vendor/session law review; AC #5 needs final-head CI, final-head sql-tdg pin, and an **independent reviewer** for every mandatory cell and parent mapping. No self-review substitutes for that role. TASK-178 remains **In Progress**, original AC #1..#5 unchecked, PR #197 draft, TASK-88/TASK-91 open.


## Scope split and bounded delivery (maintainer decision, 2026-11-10)

The original five AC above were too broad for one PR; they remain preserved verbatim in [the scope-split audit](../../docs/task-178-scope-split.md). The rewritten checked AC describe only the code, tests, evidence and explicit uncertainty that actually landed in PR #197. Earlier dated audit notes below this section reflect pre-split full-scope blockers; ownership of those blockers is now in TASK-192..197 and TASK-179/191/180, not silently waived. The status **Done** is bounded to this verified baseline and requires PR merge before its dependent TASK-192 implementation begins. All original TASK-88 acceptance and protocol 3.0.0 release gates remain open.
