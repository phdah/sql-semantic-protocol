# TASK-196 acceptance audit (implementation in progress)

Original task: `.backlog/tasks/task-196 - certify-schema-write-and-ddl-state-outcomes.md`.

## Bounded source of truth

- 38 original immutable TASK-178 cases: 11 schema, 18 DML, 9 DDL; 13 named parser dialects each, **494 cells**.
- Historical cell types are **428 C**, **11 R**, **55 U**. C means canonical-document equality, **not** vendor-native execution, strongest typed state-effects, or independent semantic approval.
- `docs/task-196-acceptance-freeze.json` pins every SQL, variant, parser cell, named source identity and review gate. `tests/task_196_acceptance_matrix.rs` rejects drift and missing original cells.
- 16 original DML and 18 original schema/DDL DuckDB closed-world source/terminal oracle programs already existed in TASK-177. `tests/task_196_original_native_states.rs` replays all 34 against the exact original SQL, including three RETURNING bags, unchanged unrelated sources, negative constraint/lifetime effects, and explicit native rejection of REPLACE/OVERWRITE.
- Original S191 (dbt manifest/catalog) and S193 (missing/conflicting schema) are authority-bound metadata sources, not fabricated native SQL poststates. `tests/task_196_metadata_authority.rs` compares direct typed DDL, catalog-introspected metadata, manifest fallback, and representable ODCS facts across 13 dialect parsers; missing metadata fails closed. This does **not** certify all mutation-side adapter parity.
- Original S199 (REPLACE INTO) and S200 (INSERT OVERWRITE) remain **unapproved** vendor-specific or unsupported physical DML outcomes, with native DuckDB syntax rejection. Parser/session/vendor disposition remains TASK-179/TASK-191.

## Acceptance tracking

| AC | Current bounded evidence | Still blocking completion |
| --- | --- | --- |
| #1 | Original 38 SQL / 494 parser cells, 34 native fixture anchors and four named boundaries frozen | Independently specify affected/rejected occurrence identities, exhaustive source-to-poststate and typed constraint/default expectations in all 38 rows |
| #2 | Original native whole source/terminal and RETURNING executions replayed via public SQL fixture | Strongest typed protocol state/effect proof against each native original, including multiplicity and negative/NULL/default cases |
| #3 | S199/S200 native rejection and explicit unapproved parser/vendor boundaries | Reviewed, release-owned dispositions with no mandatory core upgrade to approved Residual |
| #4 | Frozen SQL and two original metadata authority cases compared across direct/dbt/ODCS; TASK-197/TASK-180 handoff | Remaining 36-case source/effect adapter parity and final evidence gates |
| #5 | Task-owned tests + freeze; coverage manifest explicitly in progress | CI exact-head fmt, strict Clippy, Rust default/no-default, docs, dbt E2E, pinned sql-tdg, independent 494-cell review and evidence links |

**No signoff asserted** until all remaining required gates are independently verified. TASK-197 is blocked until this task is Done **and merged**; TASK-180, TASK-88 and TASK-91 retain their independent gates. The agent/reviewer and DuckDB fixture cannot attest other vendor execution laws.

## Implementation expansion and original semantic review (2026-11-16)

The following implementation and evidence are committed on PR #202 and require final-head CI verification before claims can be signed off:

- Original 16 DML before/after DuckDB bags, physical row identities, true/false/unknown row ordinals, partner multiplicity, affected row counts, RETURNING and typed value frequencies: `docs/task-196-original-write-occurrences.json`, `tests/task_196_write_occurrences.rs`, `tests/task_196_original_native_states.rs`, `tests/task_196_public_lifecycle.rs`.
- Newly supported bounded, source-typed `UPDATE` and `DELETE` scalar `IN (SELECT column FROM independent_source)` over complete signed-integer source bags, with sound SQL NULL handling and independently preserved source state: `src/analysis/writes.rs`, `src/physical_realization/write_states/operators.rs`, `tests/task_196_typed_in_dialects.rs`. Original untyped canonical snapshots remain explicit U, not retroactively certified. Floating, computed and unscoped membership remain residual.
- Original CTAS, ALTER, DROP and TRUNCATE complete source and row-state public API assertions in `tests/task_196_original_ddl_lifecycle.rs`, all original DDL DuckDB runtime fixtures in `tests/task_196_original_native_states.rs`. Independently declared PK/FK/UNIQUE/NOT NULL/CHECK, generated/default and DECIMAL source facts in `tests/task_196_schema_contract.rs`.
- Direct typed SQL, dbt manifest/catalog fallback, and representable ODCS facts on original 18 DML statements and the two original metadata-only reads: `tests/task_196_write_adapter_parity.rs` and `tests/task_196_metadata_authority.rs`. Adapter-provided facts do not attest runtime execution.
- `docs/task-196-independent-semantic-review.json` independently explains all 38 unchanged original SQL laws and assigns a separate explicit status to each of 494 parser cells; `tests/task_196_independent_review.rs` checks every identity and handoff. This is an agent-authored review queue, not external vendor attestation or 494 signed-off semantic cells.

### Still required before TASK-196 Done

1. Resolve all final-head CI compiler, rustfmt, strict Clippy, default/no-default test, doc and dbt E2E findings, and verify the exact final code head.
2. Perform final independent adjudication for each mandatory portable cell: strongest sound typed schema, mutation effects, complete positive/rejected/UNKNOWN row oracles, adapter support and provenance. Review rather than rubber-stamp historical C or native-only DuckDB evidence. Keep 11 original R and 55 U parser/analyzer boundaries explicitly unapproved where not proven.
3. Re-pin a compatible temporary `phdah/sql-tdg` test harness to the actual reviewed protocol commit and verify its five CI jobs; do not merge a temporary consumer harness or use stale pin evidence.
4. Record exact SHA, CI run URLs, 494 cell dispositions, explicit remaining TASK-179/TASK-191/TASK-197/TASK-180 handoffs and independent reviewer, update this audit and coverage manifest, then and only then set TASK-196 Done on the PR branch and recheck CI on signoff metadata.

Do not confuse scoped DuckDB/native portable proof with external vendor dialect execution. TASK-88 and TASK-91 remain unapproved independent parent/release gates.

## Bounded independent signoff and CI evidence

The original 38 SQL / 494 parser cells have been independently adjudicated via fixed SQL laws, native before/after, positive/rejected physical occurrence identities and typed canonical source facts. **402** C cells are approved solely for bounded source-backed portable semantics; **13** S198 conflict-order C cells and **13** S208 cascade C cells remain unapproved for TASK-179; **55** historical U and **11** R cells remain unapproved for TASK-179/TASK-191, including S199/S200/S220. No unimplemented mandatory cell is promoted to Exact or an approved Residual. The original U classifications for S203/S206 remain untyped Unsupported; separately tested authoritative signed-integer single-column IN and correlated EXISTS paths are source-backed Exact. The human/vendor execution and broad matrix gates are not signed off.

The reviewed pre-status protocol implementation is `697ba496c974d8749f670160d8d0f9a3f05e4e09` and its [CI run 43977550644](https://github.com/phdah/sql-semantic-protocol/actions/runs/43977550644) passed **9/9** jobs: fmt, strict Clippy default/no-default, both Rust test/doc configurations, dependency and dbt E2E. Temporary [sql-tdg PR #54](https://github.com/phdah/sql-tdg/pull/54) pins that exact SHA and [CI run 43978131493](https://github.com/phdah/sql-tdg/actions/runs/43978131493) passed **5/5** jobs. The task Done metadata commit is independently exercised by a new CI run and requires final exact-head consumer repinning before ready-for-review; this audit does not preapprove that unrun metadata commit.

TASK-179 owns conditional external runtime and cascade/conflict semantics; TASK-191 owns parser and unsupported native boundaries; TASK-197 owns full cross-adapter completion, TASK-180 the whole TASK-88 matrix; TASK-88 and release TASK-91 remain open. TASK-196 is Done in the bounded reviewed sense only and does not unblock TASK-197 until merged.

### Exact TASK-196 Done-state CI rerun

The independent bounded signoff-state commit `f74b2fb308862e3dec1d7fe05a3c25793f83befe` passed [9/9 protocol CI checks](https://github.com/phdah/sql-semantic-protocol/actions/runs/43978446089), including the task-file Done transition, final review and manifest. The temporary migrated consumer [PR #54](https://github.com/phdah/sql-tdg/pull/54) pinned exactly `f74b2fb308862e3dec1d7fe05a3c25793f83befe` and passed [5/5 checks](https://github.com/phdah/sql-tdg/actions/runs/43978488007). This documentary metadata update does not change canonical SQL, protocol JSON, or the physical source planner; it receives a separate final CI run. Final exact-head results are posted to PR #202. TASK-197 may start only after TASK-196 merges to main. TASK-88, TASK-91, TASK-179, TASK-191 and TASK-180 remain independently gated.
