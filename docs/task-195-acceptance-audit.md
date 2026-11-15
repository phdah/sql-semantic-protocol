# TASK-195 acceptance audit

Original source: `.backlog/tasks/task-195 - certify-window-grouping-and-ordering-outcomes.md`.

- **Scope:** 36 original SQL cases × 13 named parser dialects = 468 cells, with 446 historical canonical-equal observations, 21 canonical drift cells and one parser rejection. Historical `C` is not a typed-domain or vendor-execution proof.
- **Freeze:** `docs/task-195-acceptance-freeze.json` retains the unmodified original SQL and observed parser masks. `docs/task-195-review-queue.json` names **every one of the 468** review obligations and currently has no independent semantic approval.
- **Statement-local outcome precision:** `src/analysis/output.rs::derive_window_output_domain` now derives portable bounds for ROW_NUMBER, RANK, DENSE_RANK, NTILE with valid fixed positive buckets, PERCENT_RANK, CUME_DIST and COUNT OVER. `tests/task_195_analytic_domains.rs` checks six original numeric variants x 13 dialects and fail-closed dynamic/invalid buckets. This changes real canonical value-domain semantics, not only the evidence register.
- **Unstable physical ROW_NUMBER identities:** `tests/task_195_ambiguous_identity.rs` checks the actual public finite-window proof returns Residual for original duplicate/peer-key cases, without asserting an exact source-row selection.
- **Physical source:** six `orders` rows (including a physical NULL id and two indistinguishable SQL-value duplicates), five `customers` rows and three `products` rows. All source tuples are replayed in DuckDB.
- **Original SQL evidence:** 28 deterministic original terminals (including exact peer, fractional ranking and ROWS/RANGE/GROUPS/named-frame multiplicities); two native-rejected original spellings (`S160`, `S161`); five conditional, stochastic sampling originals (`S163`-`S167`); and one ambiguous peer-selected LIMIT (`S162`). No deterministic output is invented for the latter eight cases.
- **Physical identity and negatives:** `tests/task_195_native_partitions.rs` checks the full original QUALIFY selected/IS NOT TRUE partition (`S121`-`S124`); `tests/task_195_physical_source_instances.rs` distinguishes four physical `id=1` join pairs and one rejected `id=2` pair and checks empty global vs grouped/ROLLUP semantics and explicit NULL sort precedence. The original row-number ordering does **not** impose a total order on identical peers.
- **Source fact parity:** `tests/task_195_adapter_parity.rs` compares direct typed DDL, compiled original dbt SQL and ODCS on the 467 parser-accepted cells, without upgrading any database runtime assertion.
- **Soundness floor:** `tests/task_195_typed_soundness.rs` compares full literal native terminal bags with source-typed output value-domain containment across deterministic cases. It does not alone certify the strongest possible domain for each operator.
- **Portable document equality:** `tests/task_195_acceptance_matrix.rs` checks all 468 observations; `D` differences remain explicitly non-equivalent.

## Original acceptance criteria

| AC | Current verified work | Remaining signoff requirement |
| --- | --- | --- |
| #1 | Complete 36×13 immutable parser/SQL freeze and original native bag fixtures | Independently certified typed/positive/rejected/UNKNOWN outcomes and physical source identities in every applicable case |
| #2 | Deterministic original native outputs, QUALIFY partitions and adversarial NULL/peer/empty-source controls | Full strongest-sound domains and exact/Impossible/Residual proof of each mandatory variant, including partial order and frame boundaries |
| #3 | FETCH WITH TIES differs from LIMIT in peer-aware companion; original TOP/FETCH runtime errors remain named, sampling is conditional | Reviewed release disposition for every unresolved parser/engine/session row; TASK-179/TASK-191 retain vendor-law ownership |
| #4 | All 13 parser statuses and direct/dbt/ODCS source-fact comparisons; TASK-197/TASK-180 handoff declared | Final-head green Rust/dbt CI and exact-head pinned sql-tdg consumer compatibility |
| #5 | Source-owned code, tests, acceptance freeze, and explicit review ledger | Independent review of every bounded cell, full CI and consumer evidence; do not mark Done before verification |

**No parent or release approval:** TASK-88, TASK-91 and TASK-180 remain open; DB-specific runtime differences are not silently promoted to portable semantic proof.

## Independent bounded semantic review, 2026-11-15

- `docs/task-195-independent-semantic-review.json` independently derives all **28 original native DuckDB terminal bags from source SQL tuple algebra**, reconstructs positive/NOT TRUE candidate counts and physical row occurrence IDs, and gives a separate case-level SQL law for every one of the 36 original SQL variants. Its oracle is not computed from the protocol analyzer, and `tests/task_195_independent_review.rs` executes the original SQL and checks each reconstructed value bag and source identity.
- **468/468 parser cells reviewed individually**, with independent public outcome-domain assertions and a matching review-queue cross-check: **446** historically C cells accepted for the *bounded portable SQL analysis*, **21** canonical D cells explicitly **not approved as equivalent** and handed to TASK-191, and **one** Snowflake named-window parser rejection explicitly **not approved** and owned by TASK-191. Of the 446 C cells, **342** have original deterministic DuckDB terminal bag evidence (28 originals x 13 minus 22 D/R), and **104** have conditional or engine-rejected native execution laws (8 original cases x 13).
- The review also checks NULL/subtotal provenance, the two-by-two physical duplicate join multiplicity, ROWS/RANGE/GROUPS frames and peers, positive/rejected QUALIFY and pagination occurrence bags, three grouping-provenance cases, stochastic subset limits, and the difference between companion FETCH WITH TIES and LIMIT. No physical source peer is assigned an arbitrary ROW_NUMBER identity.
- `src/analysis/output.rs` adds the strongest source-independent bounds proven for this scoped matrix: non-null `GROUPING(id)` in {0,1}; `COUNT(1)` (normalized `COUNT(*)`) over an inclusive-current frame >=1, otherwise >=0; positive RANK/DENSE_RANK/ROW_NUMBER; bounded NTILE(3); [0,1] PERCENT_RANK and (0,1] CUME_DIST. Compiled dbt and ODCS parity is tested against direct typed DDL for all 467 accepted parser cells.
- The **review was performed by GPT-6 reconstructing original SQL algebra**, not by an external human or vendor. DuckDB is the only runtime independently executed. No TASK-88/91/180 parent, vendor, migration or release approval is implied.

Scoped TASK-195 signoff remains conditioned on matching exact-head repository CI (9 checks) and pinned migrated sql-tdg CI (5 checks), recorded in the task and manifest before the task transitions to Done.

## Bounded signoff: 2026-11-15

All five TASK-195 acceptance criteria are checked for the bounded 36 original SQL / 468 parser-cell contract. This is an agent-conducted algebra-independent semantic review, not an external maintainer or non-DuckDB vendor attestation. Original 28 native DuckDB bags, four QUALIFY positive/NOT TRUE cases, pagination and frame boundaries, and 467 source-accepted direct/dbt/ODCS parser cells have active test evidence.

- **Scoped portable analysis:** 446 reviewed parser C cells, with 342 deterministic DuckDB originals and 104 explicitly conditional/native-rejected originals. All 468 cells have a separate review decision. The 21 D plus one R cells are **reviewed but not approved as equivalent** and have a named TASK-191 handoff. No vendor-runtime fact was inferred from DuckDB or sqlparser.
- **Reviewed protocol implementation:** `af9a7e4ecedbb097fa8c10900b15f5f4a9850e69`, [9/9 CI success](https://github.com/phdah/sql-semantic-protocol/actions/runs/43764456633) including fmt, strict Clippy, both Rust feature configurations, docs and dbt E2E.
- **Pinned migrated consumer:** [sql-tdg PR #53](https://github.com/phdah/sql-tdg/pull/53) at exact protocol pin `af9a7e4ecedbb097fa8c10900b15f5f4a9850e69`, [5/5 CI success](https://github.com/phdah/sql-tdg/actions/runs/43764466918). Consumer harness is CI-only and must not merge as a release dependency.
- The task metadata signoff commit follows these green implementation runs and is separately exercised by CI. Parent TASK-88, release TASK-91, whole matrix TASK-180, cross-family adapter TASK-197, parser TASK-191 and vendor/session TASK-179 retain independent approval requirements.
