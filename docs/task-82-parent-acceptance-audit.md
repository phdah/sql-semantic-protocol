# TASK-82 independent parent acceptance review

**Status (2026-11-04): In Progress, not signed off.** This is a parent-level
cross-child verification, separate from completed child TASK-153 through
TASK-156 and separate from TASK-91's release decision.

## Integration prerequisites

All four child implementation PRs are merged into `main`:

| Child | PR | Evidence |
| --- | --- | --- |
| TASK-153 | [#174](https://github.com/phdah/sql-semantic-protocol/pull/174) | [Acceptance audit](task-153-acceptance-audit.md), matched branch precedence and NULL/duplicate cases |
| TASK-154 | [#175](https://github.com/phdah/sql-semantic-protocol/pull/175) | [Acceptance audit](task-154-acceptance-audit.md), unmatched BY TARGET/BY SOURCE partitions and first-applicable branch selection |
| TASK-155 | [#176](https://github.com/phdah/sql-semantic-protocol/pull/176) | [Acceptance audit](task-155-acceptance-audit.md), constraints, repeated matches, partial UPSERT equivalence and typed vendor residuals |
| TASK-156 | [#177](https://github.com/phdah/sql-semantic-protocol/pull/177) | [Acceptance audit](task-156-acceptance-audit.md), computed assignments and complete sequential MERGE replay |

The task-specific frozen matrices and independently checked DuckDB/dialect/adapter
oracles remain the authorities for their individual cases. Historical freeze
fields such as `mandatory_signoff=false` are not retroactively edited.

## Original parent AC crosswalk

| Parent AC | Existing child evidence | Additional parent review |
| --- | --- | --- |
| #1 Branch normalization and UPSERT boundaries | TASK-153 matched, TASK-154 unmatched, TASK-155 parser/UPSERT conditional, TASK-156 computed paths | One integrated script checks matched UPDATE, unmatched INSERT, and BY SOURCE DELETE before a computed MERGE |
| #2 Match partitions, precedence, NULL and repeated matches | TASK-153/154 TRUE vs NOT TRUE ordinals; TASK-155 ambiguous multi-match residual; TASK-156 ordered source/target preimages | Independent native DuckDB ON TRUE/NOT TRUE counts verify NULL keys remain unmatched in both directions |
| #3 Complete construction, final state and rejection/conflict | TASK-153/154/155 exact/Impossible/Residual cases and TASK-156 evolving bags | Check both intermediate target bags, unchanged physical source and complete final result |
| #4 Computed assignments and sequential MERGE | TASK-154 combinations, TASK-156 computed SET plus two-step oracles | Second statement consumes the actual first poststate; no separately invented target |
| #5 DuckDB and dialect evidence | Child native complete-bag oracles and 13-dialect parser matrices | New integrated DuckDB replay. Native non-DuckDB semantics remain unverified, not portable Exact claims |
| #6 API, protocol and adapter parity | Four frozen child suites and direct SQL, equivalent dbt/catalog and ODCS paths | Check `tests/task_82_parent_merge_program.rs` on final PR head, all required CI jobs and unchanged active JSON schema |

## Independent combined execution oracle

`tests/task_82_parent_merge_program.rs` starts with a complete, attested
`target = [(1,10),(2,20),(NULL,90)]` and
`source = [(1,100),(3,300),(NULL,500)]`, where rows are `(id, score)`.

1. `MERGE` with matched UPDATE, NOT MATCHED BY TARGET INSERT and NOT MATCHED
   BY SOURCE DELETE yields exactly `[(1,100),(3,300),(NULL,500)]`.
   The old target `NULL` row is deleted and a different source `NULL`
   row is inserted. Expected affected: **1 updated, 2 inserted, 2 deleted**.
2. A second matched MERGE computes `score = old_target.score + source.score`
   on that same poststate, yielding exactly `[(1,200),(3,600),(NULL,500)]`.
   Expected affected: **2 updated**.
3. The independent DuckDB oracle checks source and target complete bags before,
   between, and after statements; ON TRUE/NOT TRUE source and target occurrence
   ordinals; no mutation of `source`; and typed terminal INTEGER/NULL
   frequencies. The public protocol physical-state planner must agree exactly.

The frozen child oracles continue to own the complementary impossible,
ambiguous-multiple-match, FK enforcement and dialect-specific residual checks.
This new test does not certify an unexecuted native vendor engine.

## Parent closure gates

- [ ] New parent-level regression passes strict fmt, Clippy, full tests, docs,
      no-default-features, dbt E2E and active schema/protocol checks on the PR head.
- [ ] Perform separate two-axis Matt standards/spec review of the complete
      parent implementation and record findings without conflating the author
      account with a distinct reviewer.
- [ ] Independently reconcile **all six** original parent criteria against
      the executed child and combined acceptance evidence, including conditional
      vendor dispositions. Obtain owner approval for any explicit review waiver.
- [ ] Only then check original parent AC #1..#6, update the canonical coverage
      manifest and mark TASK-82 Done on the PR before merging.

TASK-83 and TASK-85 remain separately open parent gates. TASK-161 is still
blocked on TASK-82 and TASK-83 until **both** are Done and merged on `main`.
