# TASK-111 independent bounded acceptance audit

Parent: TASK-71, still In Progress. Delivery: https://github.com/phdah/sql-semantic-protocol/pull/120.

## Frozen source and terminal truth

The immutable original fixture IDs are `71-C111-P` and `71-C111-N`, with eight mandatory SQL/AST/physical bag variants in `docs/task-111-acceptance-freeze.json` (v2). The canonical constructive model proves sufficient complete physical populations at the **same source cardinalities** as the full DuckDB oracle, with the listed multiplicities; the actual frozen source values and SQL NULL keys are executed separately by the DuckDB oracle, not assumed by a schema-only witness. A synthetic output NULL is never a fabricated input row.

| Stable case | Kind and constructive pattern | Physical L ID:key | Physical R ID:key | Full terminal bag / row count | Proved physical L/R counts |
|---|---|---|---|---|---|
| `71-C111-P-LEFT-EMPTY` | LEFT, empty_right | l1:1; l2:NULL; l3:2 | ∅ | 3, l1:1:NULL:NULL; l2:NULL:NULL:NULL; l3:2:NULL:NULL | 3/0 |
| `71-C111-P-LEFT-REVERSED` | LEFT, empty_right | l1:1; l2:NULL; l3:2 | ∅ | 3, l1:1:NULL:NULL; l2:NULL:NULL:NULL; l3:2:NULL:NULL | 3/0 |
| `71-C111-P-RIGHT-EMPTY` | RIGHT, empty_left | ∅ | r1:1; r2:1; r3:NULL | 3, NULL:NULL:r1:1; NULL:NULL:r2:1; NULL:NULL:r3:NULL | 0/3 |
| `71-C111-P-FULL-EMPTY` | FULL, empty_right | l1:1; l2:NULL; l3:2 | ∅ | 3, l1:1:NULL:NULL; l2:NULL:NULL:NULL; l3:2:NULL:NULL | 3/0 |
| `71-C111-N-LEFT-MIXED` | LEFT, mixed_duplicate_right | l1:1; l2:NULL; l3:2 | r1:1; r2:1; r3:NULL; r4:9 | 4, l1:1:r1:1; l1:1:r2:1; l2:NULL:NULL:NULL; l3:2:NULL:NULL | 3/4 |
| `71-C111-N-RIGHT-MIXED` | RIGHT, mixed_duplicate_right | l1:1; l2:NULL; l3:2 | r1:1; r2:1; r3:NULL; r4:9 | 4, l1:1:r1:1; l1:1:r2:1; NULL:NULL:r3:NULL; NULL:NULL:r4:9 | 3/4 |
| `71-C111-N-FULL-MIXED` | FULL, mixed_duplicate_right | l1:1; l2:NULL; l3:2 | r1:1; r2:1; r3:NULL; r4:9 | 6, l1:1:r1:1; l1:1:r2:1; l2:NULL:NULL:NULL; l3:2:NULL:NULL; NULL:NULL:r3:NULL; NULL:NULL:r4:9 | 3/4 |
| `71-C111-N-BOTH-EMPTY` | FULL, empty_left | ∅ | ∅ | 0, ∅ | 0/0 |

Independent `tests/task_111_outer_join.rs::frozen_complete_duckdb_source_and_terminal_bags_agree` inserts each complete physical table into DuckDB, executes the SQL, sorts entire terminal row-identity bags (including duplicates and synthetic NULLs), and checks all frozen inclusive scalar bounds and NULL domains. The test also classifies **every physical Cartesian pair** using DuckDB SQL three-valued ON equality and asserts the independently computed TRUE/FALSE/UNKNOWN partition. Missing physical partners are proven against an entire source bag, not sampled pairs.

## Five acceptance criteria and evidence

1. **Frozen matrix:** All eight cases specify SQL, expected canonical single-join AST, 13 parser statuses each, source instance IDs, source columns and typed domains, complete source/terminal row identity bags, DuckDB proof, adapters and conditional laws. The 104 canonical parser cells and PostgreSQL alias are tested in `tests/task_111_outer_join.rs`. The manifest links the matrix.
2. **Canonical semantic model:** `src/constructive/types.rs` defines checked complete-population cardinality laws; `src/physical_realization/joins.rs` constructs closed physical source bags and typed positive/NOT TRUE witnesses. `disjoint_keys` proves wholly unmatched nonempty sides, `mixed_matched_unmatched` proves one matched pair plus unmatched rows, and `mixed_duplicate_right` proves two matched right partners over the frozen 3x4 bags. Output semantics are checked for LEFT/RIGHT/FULL, INNER and both SEMI/ANTI directions. The SQL input position, not the ON operand order, owns the preserved side. Two aliases of the same physical source cannot acquire incompatible histograms.
3. **Independent execution:** The eight DuckDB complete-bag fixtures include L keys [1,NULL,2] with empty R (LEFT/FULL 3), reversed equality (LEFT 3), R-only RIGHT 3, both-empty, and the mixed L=[1,NULL,2], R=[1,1,NULL,9] cases (LEFT/RIGHT/FULL = 4/4/6) with stable row IDs, matched duplicates and NULL/synthetic output separation. `tests/extended_join_witness.rs` independently verifies Exact qualifying shapes and FULL rejected=Impossible.
4. **Dialects and adapters:** All 13 exposed canonical parser dialects (plus `postgres` synonym) run the same frozen public analyzer/physical proofs and emitted canonical AST checks. `tests/task_111_outer_join_adapters.rs` compares direct SQL, compiled dbt manifest and equivalent ODCS typed source metadata for LEFT, RIGHT and FULL. Only DuckDB execution is attested; other vendors are parser-equivalent, not engine-certified.
5. **Verification/review:** `cargo fmt --check`, strict Clippy, full Rust tests, docs, no-default-features tests/lint/docs, dbt Core E2E, active canonical protocol/schema tests and focused source/terminal tests are GitHub Actions CI gates. A final-head green run is required after the signoff metadata commit; see the PR checks.

## Reviewed conditional laws

- `TASK-111-C-NULLABILITY`: A typed INTEGER catalog does not independently attest physical SQL NULL insertability. The literal NULL-containing physical bags are DuckDB execution fixtures. Any physical NULL insertion proof without authoritative metadata remains a typed Residual; no invented row is certified. Release disposition pending.
- `TASK-111-C-VENDOR`: Parser/canonical equivalence gives no attestation of native vendor engine environment (collation, implicit coercion or session semantics). Claims of native execution without such evidence remain residual; release disposition pending.
- `TASK-111-C-NULLSAFE`: NULL-safe `IS NOT DISTINCT FROM` equality is outside this child's mandatory ordinary `=` law and remains a typed Residual, checked in `tests/task_111_outer_join.rs`; TASK-113 owns the portable follow-on. Release disposition pending.

## Matt two-axis review (main...PR head)

**Standards:** Canonical joins source modeling owns the proof, not an adapter. Public integration tests use the public SQL/catalog API, conventional commits, active schema and docs. A self-join incompatible-histogram regression and mixed SEMI cardinality error were detected and corrected in this PR. Remaining conditional proof claims are bounded explicitly rather than assumed from parser success.

**Specification:** Each mandatory frozen ID has a concrete DuckDB oracle and typed source-count-compatible constructive population; the SQL operand reversal and duplicate/null-extension cases have complete output bag assertions. No later child or TASK-71 completion is claimed. Optional vendor/session or physical NULL insertability is explicitly residual, not a missing mandatory portable-equality case.

## Release boundary

This signs off **only TASK-111**, not TASK-71 or the 3.0.0 release. TASK-112 must not begin until PR #120 is merged. The release gate retains the three reviewed pending dispositions and all other child tasks.
