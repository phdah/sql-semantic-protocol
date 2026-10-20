# TASK-125 final bounded acceptance audit

Date: 2026-10-19
Owner: TASK-125. Parent: TASK-74 (still To Do).
PR: https://github.com/phdah/sql-semantic-protocol/pull/135
Frozen inventory: [task-125-acceptance-freeze.json](task-125-acceptance-freeze.json), version 2.

Version 2 explicitly repairs two omissions found in independent pre-signoff review of v1: the empty-input case needed four canonical `Empty` column domains, and every frozen SQL needed the observed per-dialect parser statuses. Neither revision adds a new semantic family or weakens a mandatory expectation.

## Independent mandatory case review

| Frozen case | Complete physical evidence and independent expected output |
| --- | --- |
| `74-C125-P1` | ASC peers [1,1,2] yield RANK [1,1,3], DENSE_RANK [1,1,2]; source identities are distinct for the duplicate rows. |
| `74-C125-N1` | QUALIFY RANK <= 1 retains the two rank-one peers and rejects source `s#2`; the complete terminal bag has two rows. |
| `74-C125-P2` | DESC ranks 2 above the peer pair of 1s; rank gaps and both terminal rank columns are exact. |
| `74-C125-P3` | Total descending order [3,2,1] assigns deterministic ROW_NUMBER [1,2,3] to physical identities [`s#2`,`s#1`,`s#0`]. |
| `74-C125-P4` | Two independent partitions reset both rank counters; full four-row bag and domains are exact. |
| `74-C125-P5` | NULLS FIRST ranks the NULL row first, then the two value-1 peers together; the x domain admits NULL. |
| `74-C125-P6` | NULLS LAST places the NULL row after the two value-1 peers, producing RANK 3; the x domain admits NULL. |
| `74-C125-N2` | QUALIFY DENSE_RANK = 2 retains only `s#2` and rejects both rank-one peers. |
| `74-C125-P7` | An empty source emits an empty terminal multiset and canonical `ValueDomain::Empty` for every projected output column. |

Both conditional cells were independently reviewed: `74-C125-C1` returns typed Residual because ROW_NUMBER peer identity is nondeterministic; `74-C125-C2` returns typed Residual for session/dialect-dependent implicit NULL ordering. A source datatype width contradiction and a duplicated complete-source identity produce Impossible, while missing evidence is Residual.

`tests/task_125_rank_bags.rs` independently reads back the complete DuckDB source bag and executes all nine mandatory SQL outputs. It checks full terminal multisets, physical positive/NOT TRUE source identities, rank gaps, NULL memberships, exact finite values, numeric inclusive extrema and empty domains against the frozen expected cells, not only against the implementation's own output.

## Child acceptance sign-off

1. **Freeze:** Both fixed fixture families, nine mandatory cells, two named conditional boundaries and their full typed source/terminal and physical membership evidence are retained in the v2 inventory, registered in `docs/coverage-manifest.json`. Exact parser classification is frozen for all 11 x 13 = 143 SQL/dialect cells. All parsed in the exposed sqlparser paths, including QUALIFY; this does not certify each vendor's native execution.
2. **Canonical semantics:** `src/physical_realization/rank_bags.rs` uses parser-independent canonical window expressions and one complete, source-schema-attested physical bag. It preserves peer multiplicity, source identity, SQL QUALIFY survivor/rejection classification, finite typed domains and Exact/Impossible/Residual, refusing unproved shapes.
3. **Independent execution oracle:** Nine mandatory complete source and output bags execute in DuckDB; the positive/negative peer fixtures, NULL rows, duplicates, rejected identities and empty input are compared in full.
4. **Dialect and adapter parity:** All 13 exposed parser dialects prove the nine mandatory cases (117 Exact physical cells) and classify the two conditional cases Residual (26 cells). `tests/task_125_rank_adapters.rs` verifies equivalent direct SQL, compiled dbt manifest and ODCS source-schema inputs produce identical 27 mandatory Exact cells and classify six conditional adapter cells as Residual. Portable parser analysis is distinct from native vendor/session execution.
5. **Final semantic-head CI and parent mapping:** Nine GitHub Actions jobs passed at `a6e619417fb63020d307bdc5ca95be348b54ba55`, [run 39325182136](https://github.com/phdah/sql-semantic-protocol/actions/runs/39325182136): cargo formatting, strict lint, full Rust tests, no-default-feature lint/test/docs, regular docs, dependency check and dbt Core E2E. The protocol's existing schema/golden snapshot tests are part of the full Rust suite. The original TASK-74 AC mapping covered here is #1, #2, #4, #5 and #6 within this child's finite ranking scope; the remaining parent variants retain later-child ownership. The final metadata-only commit still needs head CI before merge.

## Scope and release boundary

This acceptance is for the source-attested TASK-125 portable ROW_NUMBER/RANK/DENSE_RANK child, not the full TASK-74 parent. LAG/LEAD, bounded frames, aggregate/join/filtered upstream QUALIFY composition, native vendor/session laws and broader advanced window variants remain TASK-126 through TASK-128. The original TASK-74 AC remain unchecked and its release gate remains closed. No consumer-specific heuristics, dependency additions or public wire-contract changes were introduced. The PR remains unmerged until final head CI succeeds.
