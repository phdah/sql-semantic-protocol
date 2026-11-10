# TASK-192 predicate and structural-type acceptance audit

Status: **In Progress, mandatory global signoff blocked.** This file records the
bounded evidence on PR #198 without upgrading prior parser observations into
native engine certification. Original TASK-178 and TASK-88 acceptance criteria
are unchanged.

## Source of truth

- **43 original SQLs, 559 parser/canonical cells:** `docs/task-178-full-variant-inventory.json`.
- **Task-owned v1 inventory and missing-evidence fields:** `docs/task-192-acceptance-freeze.json`. Original SQL, stable IDs, and historical observed dialect statuses remain immutable.
- **40 complete predicate TRUE/FALSE/UNKNOWN DuckDB source partitions:** 25 previously independently pinned in `docs/task-177-predicate-oracles.json`, four new pattern controls in the TASK-192 freeze, and eleven newly frozen source-rowid partitions in `docs/task-192-native-partitions.json`.
- **Three non-predicate controls:** independent native overflow evaluation error S033; ordered NULL/tie terminal S042; STRUCT(a=1,b='a') scalar S179. No fabricated TRUE/FALSE partition for these non-predicate outcomes.
- **Public tests:** `tests/task_192_predicate_matrix.rs`, `tests/task_192_native_partitions.rs`, `tests/task_192_adapter_parity.rs`. Earlier separate TASK-177 native tests continue to verify the exact heterogeneous complete source values, not just row counts.

## Acceptance mapping

| Criterion | Evidence added to PR | Remaining signoff blocker |
| --- | --- | --- |
| AC #1: finite 43 × 13 versioned matrix | Complete original IDs, parser cells, linked native oracles with source-instance rowids and explicit missing facts. | Independent expected strongest typed domains and exact per-dialect semantic/oracle cells are not all frozen. Parser-equivalent C is not a semantic proof. |
| AC #2: public analyzer, exact DuckDB physical bags and type laws | 40 original predicate TRUE/FALSE/UNKNOWN/NOT TRUE closed-world source partitions, a rejected overflow original, NULL order and STRUCT scalar controls; 13-dialect public document equivalence. | End-to-end constructive physical proof and strongest safely provable typed domains are not yet independently certified for every SQL. |
| AC #3: named families, full wire outcomes, parser boundaries | All 43 original SQL records and 559 observed canonical cells retained; STRUCT parser exceptions are explicit; original native scripts are executed rather than synonyms. | No global independent expected-document semantic review; vendor collation, timezone, NaN and structural type laws are conditional. |
| AC #4: source adapter facts / handoff | Direct SQL, dbt compiled/source metadata, ODCS source-schema parity tests for 41 source-bearing queries × 13 parser dialects; no artificial adapter claim for overflow/STRUCT scalars. | Final TASK-197 cross-family parity handoff and pinned sql-tdg consumer compatibility remain unverified. |
| AC #5: final-head CI and independent review | GitHub Actions executes fmt, strict Clippy, default/no-default Rust tests/docs and dbt E2E. | Require final-head green CI, independent cell-by-cell review, active schema/golden checks, and relevant pinned consumer evidence before Done. |

## Soundness boundaries and release ownership

A source bag and a selected terminal are distinct from a general domain proof:
the test input bag does not constrain arbitrary future relation data. When a
query projects `id` but only filters `name`, `amount` or `created_at`,
a value restriction on the filtered source column must **not** be misapplied
as an `id` domain. Nullability, numeric widths, signed range endpoints and
exclusion directions must be checked from typed public outcome evidence.
Unknown is not Exact, and an accepted parse is not native execution.

The original S033 fixed-width overflow has no successful terminal row. S042
uses a fixed DuckDB NULLS LAST session and a complete ordered terminal, without
inventing stable identities for underordered peers. S179 has a positive native
STRUCT scalar and explicit nonportable parsing/typed-output boundaries.
S026/S038 locale-aware string comparisons, S035/S037 timezone/DST boundaries,
and S040/S041 NaN/signed zero preserve the actual native DuckDB evidence only.

- **TASK-179** owns external vendor/session laws; no approval is implied here.
- **TASK-191** owns parser/Unsupported dispositions, notably STRUCT.
- **TASK-197** owns independent cross-adapter parity handoff.
- **TASK-180** owns independent whole-matrix certification.
- **TASK-88/TASK-91** remain unsigned release gates.

No missing mandatory core semantic behavior is an approved Residual.
Only mark TASK-192 Done after every AC #1..#5 has direct executable evidence
and independent verification against the frozen SQL, type, source and terminal
matrices, plus passing exact-head CI.

## Additional typed-policy evidence

`docs/task-192-acceptance-freeze.json` now assigns every original variant an independent expected-output policy: 10 exact scalar set/interval and bound-direction examples (S001–S009 and S012), 31 unconstrained nullable `id` projections, one native fixed-width overflow error (S033), and one conservative STRUCT CAST boundary (S179). These policies cover all 559 parser cells as an invariant floor, but parser coverage must not be confused with independently certified strongest typed semantics.

`tests/task_192_typed_soundness.rs` checks that all 41 successful original numeric terminals fit the emitted domains on every parser, including NULLs, duplicates and signed values, and ensures no unsupported finite `id` restriction is inferred from predicates on other source columns. The test is evidence of soundness on the pinned native samples, **not** completeness over arbitrary source bags.

The temporary [sql-tdg consumer compatibility PR #49](https://github.com/phdah/sql-tdg/pull/49) has an exact protocol commit pin for CI only. The main consumer's published dependency must remain unchanged.
