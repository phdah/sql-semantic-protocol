# TASK-156 computed and sequential MERGE acceptance audit

Scope: **TASK-156**, child 4/4 of TASK-82, and not a parent TASK-82 or protocol v3 release approval.

## Frozen ownership and finite evidence

- Current accepted-for-testing contract: [v4](task-156-acceptance-freeze-v4.json), activated by [task-156-acceptance-freeze.json](task-156-acceptance-freeze.json). Immutable [v1](task-156-acceptance-freeze-v1.json), [v2](task-156-acceptance-freeze-v2.json) and [v3](task-156-acceptance-freeze-v3.json) preserve earlier evidence.
- v2 restores the originally required enforced-constraint contradiction, complete source-side TRUE / NOT TRUE occurrence partitions, and explicit vendor parser classification that v1 lacked.
- v3 corrects the independently observed **parser boundary** for `MERGE OUTPUT` in all 13 dialects. It is not a claim that these dialects execute the same OUTPUT law.
- v4 corrects an inconsistent frozen source preimage for the second MERGE in the nested-stage case. The independent DuckDB oracle showed that stage score=2 after the first statement, not the initial score=1. This correction preserves original terminal and SQL semantics.
- Six task-owned named row-state cases, each with complete typed prestate/source bag and nullability, inclusive min/max and exact duplicate frequencies: **four Exact** programs, **one Impossible** enforced PRIMARY KEY collision and **one Residual** duplicate-match winner boundary.
- Two-step programs include incremental computed matched SET, computed NOT MATCHED INSERT followed by matched SET, complete prior-target-as-source composition, and SQL NULL equality that does not match.
- Extra transaction/session conditional boundary is explicitly typed **Residual** (owner TASK-156), not an unproved portable commit/rollback rule.

## Original child AC reconciliation

| AC | Concrete proof or remaining gate |
| --- | --- |
| #1 | Frozen fixture IDs `82-C156-P/N`; six complete cases; v1/v2/v3/v4 history; exact and conditional expected dialect/adapters; `tests/task_156_acceptance_inventory.rs`. |
| #2 | Reuse canonical `src/physical_realization/write_states.rs` and `operators.rs::merge_rows`, including evaluated expressions against old target rows, validated enforced keys, ordered new target sources, explicit impossibility and ambiguous-match residual. |
| #3 | `tests/task_156_sequential_merges.rs` runs independent DuckDB complete pre/post/source states, row ordinals, both directional TRUE/NOT TRUE partitions, typed domains, affected counts and terminal full bags; includes native rejected/NULL/duplicate and enforced collision evidence. |
| #4 | `tests/task_156_dialect_matrix.rs` guards 65 exact parser/canonical verdicts over 13 dialects and vendor boundaries. `tests/task_156_adapter_parity.rs` compares every frozen proof over directly supplied catalog facts, parsed dbt manifest/catalog facts, and ODCS v3.2 metadata. Stage and terminal evidence is explicitly supplied for dbt where the preexisting artifacts contain no such nodes; no invented dbt materializations are claimed. |
| #5 | GitHub Actions CI on final PR head, strict rustfmt/Clippy, default and no-default test/doc, dbt Core E2E, existing protocol/schema snapshots, and independent code-review still require final verification before status Done. |

## Original TASK-82 parent AC mapping (parent remains open)

| Parent AC | Child coverage |
| --- | --- |
| #1 | TASK-153/154 first-applicable match kinds, TASK-155 vendor UPSERT residual; TASK-156 computed branches and dialect classifications |
| #2 | TASK-153/154/155 multiplicity, NULL and order; TASK-156 closed TRUE/NOT TRUE partitions on every step |
| #3 | TASK-155 conflict/negative outcomes and TASK-156 evolving initial/intermediate/source/terminal full bags |
| #4 | TASK-154 branch combinations and TASK-156 computed SET/INSERT plus repeated MERGE execution |
| #5 | TASK-153/154/155 oracles; TASK-156 full DuckDB replay and 13 parser-only identities |
| #6 | TASK-156 versioned regression and adapter/dialect matrices, retaining active protocol/schema contract, plus previously completed child coverage |

## Review and release boundary

- No public protocol, JSON schema, library API or consumer-specific change is made by these verification-only additions.
- Vendor-native UPSERT winners, `MERGE OUTPUT`, session ordering and duplicate-match errors remain explicit conditional boundaries; parser acceptance is never mistaken for runtime equivalence.
- Independently verify this PR's standards and spec axes against `AGENTS.md`, `CONTRIBUTING.md` and the original TASK-156/82 AC. A self-authored test audit does **not** stand in for independent review.
- Do not mark TASK-156 Done or approve the parent until final-head CI and the required independent review have been verified. The downstream TASK-91 release gate is separate.
