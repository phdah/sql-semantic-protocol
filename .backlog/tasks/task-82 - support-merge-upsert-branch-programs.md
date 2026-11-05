---
id: TASK-82
title: Prove complete MERGE and UPSERT branch/state semantics
status: Done
assignee: []
created_date: '2026-10-09'
updated_date: '2026-11-05'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-75
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-153
  - TASK-154
  - TASK-155
  - TASK-156
references: 
  - 'TASK-65'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current MERGE only carries conditional effects and leaves match multiplicity, branch selection and conflicts residual.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Normalize ordered WHEN MATCHED/NOT MATCHED BY TARGET/BY SOURCE clauses, UPDATE/DELETE/INSERT actions, additional predicates, ON expressions and dialect UPSERT equivalents.
- [x] #2 Prove complete source-target match partitions and branch precedence, repeated source matches, target uniqueness, NULL match semantics and ambiguous engine behaviors.
- [x] #3 Expose source and initial target construction plus exact final rows and independent rejected/nonmatching cases with conflict and idempotence outcomes.
- [x] #4 Cover computed assignments, UPDATE/DELETE/INSERT branch combinations, partial target constraints and multiple sequential MERGEs.
- [x] #5 DuckDB execute complete before/action/after snapshots, then versioned per-dialect syntax and available-engine checks; no promise of portability where semantics differ.
- [x] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-82 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: MERGE matched UPDATE/DELETE, NOT MATCHED INSERT and BY SOURCE handling in ordered first-applicable branches, with full target/source snapshot.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: computed assignments and two sequential MERGEs, NULL key and unique vs repeated matches with exact or impossible/residual strength.
3. **Conditional and non-exact boundary:** Inventory conditional: dialect UPSERT equivalents, ambiguous many-to-one matches and engine-specific branch semantics; exact only with proven match/cardinality constraints.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Bounded dependency-ordered child implementation queue (2026-10-17)

TASK-82 is a **parent signoff**, not a single implementation PR. Preserve all original AC and only sign them off independently after all children are Done and merged. Pick the first unfinished dependency-ready child. Parent depends on the children only for closure; no child depends on its parent. Scope and fixed fixture IDs are in `docs/backlog-child-queue-plan.json`.

- **[TASK-153](task-153%20-%20prove-ordered-merge-matched-update-delete-actions.md)** (1/4): MERGE ON unique key; MATCHED UPDATE/DELETE with WHEN predicates; NULL key. IDs `82-C153-P`, `82-C153-N`.
- **[TASK-154](task-154%20-%20prove-unmatched-insert-and-by-source-branches.md)** (2/4): NOT MATCHED BY TARGET INSERT; NOT MATCHED BY SOURCE UPDATE/DELETE; branch order. IDs `82-C154-P`, `82-C154-N`.
- **[TASK-155](task-155%20-%20classify-repeated-matches-upsert-equivalents-and-constraint-effects.md)** (3/4): ambiguous many-to-one MERGE; two NULL keys; unique FK; dialect ON CONFLICT. IDs `82-C155-P`, `82-C155-N`.
- **[TASK-156](task-156%20-%20compose-computed-and-sequential-merges-and-certify-task-82.md)** (4/4): computed SET/INSERT; two MERGEs; nested prior state; dialect matrices. IDs `82-C156-P`, `82-C156-N`.

**Original AC mapping:** #1 -> TASK-153, TASK-154, TASK-155, TASK-156; #2 -> TASK-153, TASK-154, TASK-155, TASK-156; #3 -> TASK-153, TASK-154, TASK-155, TASK-156; #4 -> TASK-154, TASK-156; #5 -> TASK-153, TASK-154, TASK-155, TASK-156; #6 -> TASK-153, TASK-154, TASK-155, TASK-156.

**No premature completion:** Each child freezes and verifies its own exact expected source/target data, output domains, dual-direction witnesses, parser/dialect/adapter cells, and final CI; the parent gets separate whole-AC closure. A conditional vendor limitation must name an owner and approved evidence boundary; it cannot hide a mandatory unimplemented family.

## Independent parent closure in progress (2026-11-04)

All four dependency children TASK-153..156 are marked Done and their implementation PRs
[#174](https://github.com/phdah/sql-semantic-protocol/pull/174),
[#175](https://github.com/phdah/sql-semantic-protocol/pull/175),
[#176](https://github.com/phdah/sql-semantic-protocol/pull/176) and
[#177](https://github.com/phdah/sql-semantic-protocol/pull/177) are merged into `main`.

The [independent parent acceptance crosswalk](../../docs/task-82-parent-acceptance-audit.md)
links each original AC to its child proof and introduces a frozen, parent-level mixed
MERGE/DuckDB replay. This **does not** automatically sign off the original criteria.
Parent AC #1..#6 remain unchecked until PR-head CI, independent parent review, and
all mandatory/conditional disposition gates are verified. The earlier child-specific
reviewer exceptions do not waive independent TASK-82 closure. TASK-84 and TASK-91
remain blocked and unsigned.

## Parent technical completion evidence (2026-11-04)

- The active parent v3 freeze, with preserved v1/v2 histories, checks 38
  child-owned proof cases without reclassification (23 Exact, 5 Impossible,
  10 Residual), all six original AC mappings, 13 parser dialect identities,
  and equivalent available SQL/dbt/ODCS evidence.
- Four parent fixtures independently exercise native DuckDB two-step MERGE,
  TRUE/NOT TRUE membership, complete source/target bags, INTEGER/NULL domains,
  impossible/residual boundaries, bounded idempotence and non-idempotence.
  The reviewed correction counts two updates on the unchanged second MERGE.
- The implementation head `2f5b4c8157f37e2d018a5f99d6d93f53f7cb6ef0`
  passed [all nine CI jobs](https://github.com/phdah/sql-semantic-protocol/actions/runs/41925632614).
- Parent Standards/Spec independent-review identity remains unverified. No
  child review waiver extends to this parent, and no separate account
  approval is claimed. Thus original AC #1..#6 stay unchecked and status
  remains **In Progress** despite technical evidence completion. Require
  a distinct independent reviewer or explicit owner exception before Done.
- TASK-91 is a separate release decision; vendor-native, FK/UNIQUE and
  transaction laws remain typed conditional boundaries, never Exact by default.

## Parent signoff completed (2026-11-05)

- Original TASK-82 AC #1..#6 signed off as **Done** against the finite parent
  acceptance crosswalk, frozen child evidence (TASK-153..156), and four
  independently executed parent DuckDB fixtures.
- Final code review used Matt `engineering/code-review` Standards/Spec axes;
  the final verified implementation head `8cf1692c2adf51bfff13517c4c17281840ab14dc`
  passed [9/9 CI jobs](https://github.com/phdah/sql-semantic-protocol/actions/runs/41985740438).
- Owner approval in [PR #186](https://github.com/phdah/sql-semantic-protocol/pull/186#issuecomment-6652370938)
  grants an **explicit TASK-82-specific exception** to the separate independent
  reviewer identity requirement. This is an owner exception, not an independent
  review and not inherited from any child task.
- Accepted limits remain typed Conditional/Residual, including vendor UPSERT,
  duplicate-match winner/error laws, nullable constraints, transaction/session
  semantics and unresolved native dialect equivalence. No v3 release approval
  or TASK-91 signoff is implied.
