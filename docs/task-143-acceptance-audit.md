# TASK-143 finite producer composition acceptance audit

**Status: In Progress. Not signed off.** Parent TASK-78 stays open.

## Bounded implementation

- `78-C143-P-JOIN`: one complete physical `raw` bag, `k > 1` and checked `k + 10` transformation, shared across two aliases of one lexical producer; complete source/producers/terminal identities and typed terminal domains.
- `78-C143-N-NULL-DUP`: NULL comparison is UNKNOWN and `k=1` is FALSE; duplicate physical source occurrences produce four self-join pairs and absent tuples are Impossible, without synthetic source insertions.
- `78-C143-P-COUNT`: `COUNT(*)` over the shared filtered producer JOIN has a complete closed-world output including 0 rows, duplicate multiplicity, a precise nonnullable output domain and absent-count proof.
- Public API oracles: `tests/task_143_filtered_producers.rs`, `tests/task_143_count.rs`. Independent DuckDB complete bag oracles: `tests/task_143_duckdb_oracles.rs` and `tests/task_143_count.rs`. Exposed dialect parser paths: `tests/task_143_dialects.rs`.
- Parser-independent facts are in `src/protocol/cte.rs`, the AST boundary is `src/analysis/cte.rs`, and complete physical evaluation is `src/physical_realization/cte_bags.rs`. Unsupported expression/precision shapes remain explicit Residual.

## Outstanding acceptance blockers

- Original TASK-143 AC #2/#3: downstream **ordered write** from the filtering/computed materialized CTE has not been integrated with canonical write-source state semantics or compared against a full DuckDB oracle.
- AC #1/#4: the full reviewed fixture/variant inventory and SQL/dbt compiled/ODCS evidence-source parity are incomplete. Parser names are exercised; session/native vendor semantics are not certified.
- AC #5: final-head implementation CI, schema/golden checks, the independent Standards/Spec review and exact mapping of every mandatory matrix cell remain unverified. No child or TASK-78 parent signoff is claimed.

No missing mandatory requirement is treated as an approved residual. TASK-144 cannot start until this child is Done and merged on main.
