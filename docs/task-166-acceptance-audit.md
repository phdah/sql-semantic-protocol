# TASK-166 acceptance audit (2026-11-03)

**Owner:** TASK-166, child 2/4 of TASK-85. **Parent status:** open. **Freeze:** `docs/task-166-acceptance-freeze.json` v1, committed before the first semantic implementation.

## Proven implementation scope

- The proof in `src/physical_realization/joint_distributions.rs::shared_filtered_histogram_plan` uses a **single** complete `SourceRows` assignment. The transparent terminal fixes the physical source count and complete typed distribution. A directly filtered terminal reuses that exact physical column and evaluates SQL TRUE on every typed histogram bucket, preserving duplicates and SQL NULL rejection.
- A filtered output requiring more occurrences than the complete source is Impossible. A contradictory filtered count or projected complete histogram is Impossible **only** when the underlying exact predicate and shared column identities are verified. Missing typed/schema evidence, unassigned filter columns, joins and non-copy outputs remain Residual.
- The existing joint-histogram path is reused for transparent/transparent terminals; the new path cannot multiply independent sufficient sources or fabricate rows in derived relations. Proofs retain a canonical physical source identity, closed-world obligations and exact OutputRows targets for both queries.

## Mandatory and conditional cases

| Frozen case | Expected | Evidence | Status |
| --- | --- | --- | --- |
| `85-C166-P-shared` | Exact count 3 and filtered count 2, physical `[1,1,2]` | `tests/task_166_shared_goals.rs`, DuckDB duplicate bag | Code + test added |
| `85-C166-P-null` | Exact NULL-aware filtered count 1 | `tests/task_166_shared_goals.rs`, DuckDB NULL bag | Code + test added |
| `85-C166-N-upper` | Impossible count 4 from complete source count 3 | `tests/task_166_shared_goals.rs` | Code + test added |
| `85-C166-N-histogram` | Impossible `WHERE k = 1` output bag `[2,2]` | `tests/task_166_shared_goals.rs` | Code + test added |
| `85-C166-C-opaque` | Residual, unknown function semantics | `tests/task_166_shared_goals.rs` | Conditional |
| `85-C166-C-missing-schema` | Residual, unattested source type | `tests/task_166_shared_goals.rs` | Conditional |

The 13 standard dialect identities are exercised by the public API tests for the portable cases. `tests/task_166_adapter_parity.rs` checks typed direct SQL, compiled dbt plus catalog, and (when enabled) ODCS source metadata against the same shared count goals. No vendor-native execution is claimed.

## Unclosed original acceptance gates

1. **AC #1:** The v1 named variants and 78 parser cells are frozen, but their individual independently verified statuses and full per-cell typed domains/NULL/inclusivity evidence must be recorded before signoff.
2. **AC #2:** The positive and impossible shared-source subsets are implemented. Broader correlated goals, additional source columns, and unproved expressions must remain deliberately Residual; assess exact terminal domains and proof identity in an independent review.
3. **AC #3:** Focused DuckDB full-bag oracles exist, but a frozen-case-driven public-API inventory that compares all source/terminal snapshots with emitted `to_bundle_json` typed claims and TRUE/NOT TRUE identities is still required for full acceptance.
4. **AC #4:** Direct/dbt/ODCS parity tests are added. Record each dialect/parser and adapter cell's CI result and a reviewed, named vendor/session/external-consumer conditional disposition.
5. **AC #5:** Require **final-PR-head** fmt, strict Clippy, full default/no-default Rust tests, docs, dbt E2E and relevant pinned sql-tdg consumer qualification. Complete independent standards/spec review of the final diff and source cells, then owner signoff. Do **not** mark TASK-166 Done before all gates close. TASK-85 remains open.

No independent-review approval, native vendor proof or final-head CI success is asserted by this audit.
