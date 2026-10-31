# TASK-81 independent parent acceptance audit

**Status:** Parent acceptance crosswalk frozen and review started. Original TASK-81 AC #1..#6 remain unchecked until the complete implementation-head CI and two-axis review pass. No independent release or native vendor certification is claimed.

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

**Gate:** The MERGE replay and nine-job CI must actually pass before parent AC #4 signoff. TASK-82 still owns general MERGE branch programs; this parent replay does not advance that separate task.

## Verification and pending signoff

Child acceptance CI: [TASK-149](https://github.com/phdah/sql-semantic-protocol/actions/runs/40873925034), [TASK-150](https://github.com/phdah/sql-semantic-protocol/actions/runs/40916998581), [TASK-151](https://github.com/phdah/sql-semantic-protocol/actions/runs/41022690530), [TASK-152](https://github.com/phdah/sql-semantic-protocol/actions/runs/41051488486). Each child records successful verification for its implementation head and has since merged.

Required fresh PR-head checks: `cargo fmt --check`, strict Clippy, `cargo test`, Rust docs, tests/lint/docs without default features, active JSON/schema snapshots and dbt Core E2E. Independent Standards/Spec code review must verify the original six AC, not only inventory totals. Until those pass, TASK-81 stays **In Progress**, all six original AC unchecked, `parent_signed_off=false`.

TASK-153 remains blocked until this parent is **Done and merged**. TASK-91 remains a separate release gate.

**Skill:** Matt Pocock `implement`; frozen parent data seam reviewed using the `tdd` guidance; final `code-review` gate pending.
