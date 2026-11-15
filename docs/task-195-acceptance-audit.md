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
