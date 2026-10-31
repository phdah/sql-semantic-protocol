# TASK-81 independent parent acceptance audit

**Status:** All six original TASK-81 criteria signed off for the bounded portable contract after full implementation-head CI, original-criterion review, and independent DuckDB parent replay. The PR still requires green metadata-head CI and merge; this does not authorize protocol 3.0.0 or native multi-vendor claims.

## Dependency/merge gate

All four bounded child tasks are **Done and merged** on `main`:

| Child | Merged PR | Frozen version | Mandatory / conditional |
| --- | --- | --- | --- |
| TASK-149 | [#161](https://github.com/phdah/sql-semantic-protocol/pull/161) | v2 | 13 / 3 |
| TASK-150 | [#162](https://github.com/phdah/sql-semantic-protocol/pull/162) | v3 | 8 / 2 |
| TASK-151 | [#163](https://github.com/phdah/sql-semantic-protocol/pull/163) | v3 | 7 / 2 |
| TASK-152 | [#164](https://github.com/phdah/sql-semantic-protocol/pull/164) | v4 | 6 / 5 |

The [v2 independent parent freeze](task-81-parent-acceptance-freeze.json) indexes **34 mandatory** and **12 conditional** entries, including the standalone TASK-149 and TASK-152 reviewed boundary inventories. It preserves 27 Exact, 3 Impossible and 16 Residual classifications with their child-owned evidence.

## Original acceptance review

| Original AC | Evidence owned by completed child contracts | Mandatory review boundary |
| --- | --- | --- |
| #1 UPDATE/DELETE semantics | TASK-149 literal/computed/default and RETURNING, TASK-150 UPDATE FROM/DELETE USING and CTE, TASK-151 correlated and qualified targets | Vendor ORDER/LIMIT/partition and unsupported RETURNING forms remain explicit Residual |
| #2 Source row selection | TASK-149 SQL TRUE/FALSE/UNKNOWN and simultaneous assignments, TASK-150 duplicate and NULL join populations, TASK-152 ordered preimage/postimage | Multiple matching UPDATE source rows cannot select an invented deterministic winner |
| #3 Pre/post and impossible states | TASK-149 enforced-key contradiction, TASK-150 complete bags, TASK-151 typed RETURNING, TASK-152 enforced CHECK/PK impossibility | Missing complete source/attestation cannot produce a fabricated target bag |
| #4 Ordered scripts and RETURNING | TASK-152 three statements from empty target to surviving updated row, TASK-151 UPDATE/DELETE RETURNING | General MERGE belongs to TASK-82. Native transaction/rollback timing is not attested |
| #5 Native and correlated tests | Child public API and full physical DuckDB source/terminal oracles, including unmatched/duplicate/NULL/CTE/correlation | No native 13-engine claim from parser coverage |
| #6 Contract and parity | Child 13-dialect parser matrices, SQL/dbt compiled catalog/ODCS evidence and active protocol/schema tests | Equivalence requires authoritative metadata; native vendor execution deferred |

`tests/task_81_parent_closure.rs` independently checks the fixed crosswalk against all four checked-in child inventories: unique IDs, exact case/fixture ownership, no Residual→Exact promotion, full target bags and typed domains for all Exact cases, vendor disposition, 13 dialect identities, and 3 adapter sources. It does not replace the child-owned native execution assertions.

## Parent-level findings

- The ambiguous multi-match UPDATE, vendor ORDER/LIMIT/partition laws, non-attested defaults/casts and missing source populations remain correctly **Residual**, with explicit source or vendor evidence requirements. These are not waived mandatory exact features.
- RETURNING unbound alias or wildcard without schema is a mandatory **negative safety** case. No invented terminal domain should be treated as a successful write.
- TASK-152's three-write case has a complete target: `[] → [(1,7,10),(2,7,20)] → [(1,7,15),(2,7,20)] → [(1,7,15)]`. It is not a proof of arbitrary MERGE or transaction rollback.
- TASK-152's native CHECK/PK failures are **Impossible** with independently preserved last committed state. FK deferral remains a reviewed engine/session boundary.
- Production SQL semantics and protocol wire shapes remain unchanged in this parent audit.

## Parent-level original AC #4 replay (v2)

The original v1 crosswalk is [archived](task-81-parent-acceptance-freeze-v1.json) without altering any child case. The v2 inventory independently freezes `81-P-REPLAY-MERGE` and `81-N-REPLAY-MULTIMATCH` in `tests/task_81_parent_merge_replay.rs`.

- **Exact bounded positive:** Complete initial target `[]` and immutable source `[(1,50),(3,30)]`. INSERT yields `[(1,10),(2,20)]`; uniquely matched MERGE UPDATE key 1 and INSERT key 3 yields `[(1,50),(2,20),(3,30)]`; UPDATE RETURNING key 3 yields `[(1,50),(2,20),(3,35)]` and `[(3,35)]` with exact typed inclusive bounds. One shared target population is evolved in statement order, never injected at the intermediate stage. The test compares all canonical and independent DuckDB source/target bags and numeric domains.
- **Negative safety:** Duplicate source keys 1 with conflicting scores 50/60 are Residual, not a fabricated chosen winner. No native multi-match law is claimed.

**Evidence:** Both frozen parent replay tests passed with and without default features in [nine-job CI run 41248576810](https://github.com/phdah/sql-semantic-protocol/actions/runs/41248576810). TASK-82 still owns general MERGE branch programs; this parent replay does not advance that separate task.

## Verification and independent parent signoff

Child acceptance CI: [TASK-149](https://github.com/phdah/sql-semantic-protocol/actions/runs/40873925034), [TASK-150](https://github.com/phdah/sql-semantic-protocol/actions/runs/40916998581), [TASK-151](https://github.com/phdah/sql-semantic-protocol/actions/runs/41022690530), [TASK-152](https://github.com/phdah/sql-semantic-protocol/actions/runs/41051488486). Each child records successful verification for its implementation head and has since merged.

**Implementation-head verification:** [All nine CI jobs succeeded](https://github.com/phdah/sql-semantic-protocol/actions/runs/41248576810) at `c3fa5c2b7951b11428f8873ab029783b8d56c8f1`, including rustfmt, strict Clippy, complete Rust tests and docs with and without default features, dependency checks, dbt Core E2E and active protocol/schema snapshots. Logs explicitly show `tests/task_81_parent_closure.rs` and `tests/task_81_parent_merge_replay.rs` each passing 2/2 tests in both feature configurations.

**Matt code-review, fixed point `main`:** [Standards and Specification COMMENT](https://github.com/phdah/sql-semantic-protocol/pull/173#pullrequestreview-5725282087). Standards: 0 hard findings; parser/analysis/emission boundaries, typed domain proofs, canonical source-independent adapter facts and immutable child freezes preserved. Spec: the original AC #4 gap was resolved by the v2 INSERT/MERGE/UPDATE replay; each original AC #1..#6 is independently traceable through the 46 child cells and two parent replay oracles. General MERGE is owned by TASK-82; vendor runtime claims remain explicit Residual, not a missing portable mandatory cell. This is a self-authored review COMMENT, not a distinct-account GitHub approval.

**Signoff decision (2026-10-31):** Original TASK-81 acceptance criteria #1..#6 are satisfied for the reviewed finite portable contract and TASK-81 is **Done** in this PR. The immutable v1 crosswalk is retained; v2 expands only the parent AC #4 replay, without altering the frozen child cases. A final **metadata-head** nine-job CI run must pass before merging; a task marked Done but unmerged does **not** unblock TASK-153. No release approval follows.

TASK-153 remains blocked until this parent is **Done and merged**. TASK-91 remains a separate release gate.

**Skills:** Matt Pocock `implement` and `code-review` (Standards/Spec), using test-first verification at the frozen acceptance seam.
