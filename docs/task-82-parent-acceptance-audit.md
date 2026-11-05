# TASK-82 independent parent acceptance crosswalk

**Scope:** The original TASK-82 AC #1..#6, independently of the completed
TASK-153..156 child acceptance criteria. This is a verification PR, not an
authorization to merge protocol v3, and is not itself an independent reviewer
sign-off.

## Merged prerequisites

| Child | Merged PR | Frozen contract | Executable evidence |
| --- | --- | --- | --- |
| TASK-153 | [#174](https://github.com/phdah/sql-semantic-protocol/pull/174) | `docs/task-153-acceptance-freeze.json` v3 | `tests/task_153_merge_matched.rs`, `tests/task_153_dialect_adapter.rs` |
| TASK-154 | [#175](https://github.com/phdah/sql-semantic-protocol/pull/175) | `docs/task-154-acceptance-freeze.json` v4 | `tests/task_154_merge_unmatched.rs`, `tests/task_154_dialect_adapter.rs` |
| TASK-155 | [#176](https://github.com/phdah/sql-semantic-protocol/pull/176) | `docs/task-155-acceptance-freeze.json` v4 | `tests/task_155_merge_conflicts.rs`, `tests/task_155_fk_unique.rs`, `tests/task_155_dialect_adapter.rs` |
| TASK-156 | [#177](https://github.com/phdah/sql-semantic-protocol/pull/177) | `docs/task-156-acceptance-freeze.json` v4 | `tests/task_156_sequential_merges.rs`, `tests/task_156_dialect_matrix.rs`, `tests/task_156_adapter_parity.rs` |

Each child is `Done` on `main`. The review exceptions recorded in the
individual child tasks apply to those children only, never to this parent.

## Independent original AC reconciliation

| Parent AC | Evidence to audit at parent boundary |
| --- | --- |
| #1 Ordered branch normalization and dialect UPSERT | TASK-153 matched first-applicable UPDATE/DELETE, TASK-154 unmatched target/source, TASK-155 typed ON CONFLICT boundaries, TASK-156 computed branches. Parser support is not native dialect-equivalence proof. |
| #2 Complete source/target partitions | Child native TRUE/NOT TRUE occurrence partitions, NULL keys, source multiplicity, enforced uniqueness and conditional repeated-match law. The new parent replay checks both source and target membership at **each** step. |
| #3 Initial, final and rejected physical populations | TASK-153/154 negative fixtures and TASK-155 conflicts; TASK-156 multi-write replay. New `82-PARENT-MIXED-P` asserts both complete target populations and untouched shared source after two MERGEs; `82-PARENT-MULTIMATCH-N` keeps duplicate matches Residual. |
| #4 Computed actions and sequential MERGEs | TASK-154 three-way matched/unmatched combination and TASK-156 computed assignments. New parent replay combines computed update, unmatched insertion, by-source deletion, matched deletion and a second computed MERGE. |
| #5 Native and dialect evidence | Child-specific full DuckDB before/after oracles; native two-step parent replay against an independent frozen bag. Existing child matrices enumerate 13 parser dialect identities, not 13 native engines. |
| #6 Tests, docs, schema and adapters | Child acceptance inventories, default/no-default Rust test suites, strict lint, dbt E2E, direct SQL/dbt/ODCS parity and protocol/schema regressions. No new public protocol shape or API is introduced here. |

## Parent-owned finite oracle (active v3, immutable v1/v2 history)

`docs/task-82-parent-acceptance-freeze-v1.json` preserves the initial two-case inventory. The active v3 adds complete 38-case child reconciliation and two bounded repeat-application cases. The original v1 fixes two
fixtures before their test implementation:

- `82-PARENT-MIXED-P`: initial target `[(1,10),(2,20),(NULL,8)]`;
  source `[(1,5),(3,30)]`. First MERGE yields
  `[(1,15),(NULL,8),(3,30)]` with one UPDATE, INSERT and DELETE.
  Second MERGE yields `[(NULL,8),(3,40)]` with one UPDATE and DELETE.
  Each step freezes the exact INTEGER/NULL domain, bounds, inclusive
  endpoints, complete source/target TRUE/NOT TRUE counts, and full bags.
- `82-PARENT-MULTIMATCH-N`: two matching source rows for one target,
  which must be `Residual(UnprovedOperation)`, not a fabricated winner.

`tests/task_82_parent_merge.rs` uses the public analyzer and physical write
plan, compares the frozen pre/post states against separately executed DuckDB
statements, and never treats an intermediate target as independently sourced.

## Conditional boundaries and unfinished gates

The parent does **not** certify arbitrary native UPSERT equivalence, MERGE
OUTPUT behavior, duplicate-match winner/error rules, vendor-specific NULL
uniqueness or FK deferral, or transaction/session behavior. TASK-155 owns
the named conditional cases; TASK-84/87 own further enabling evidence.
A parser-accepted statement is never native-engine proof.

The parent review was initially pending PR-head CI and a distinct reviewer
identity. On 2026-11-05, owner approval recorded a TASK-82-specific exception
to that identity gate after successful review and CI; no child waiver was
inherited. Original AC #1..#6 are now signed off under that explicit exception.
TASK-83, TASK-85 and TASK-91 retain their own independent closure, and
TASK-161 still requires TASK-83 to be Done and merged.


## Full child inventory reconciliation (active v3)

The parent v3 freeze indexes **38** child-owned cases, preserving their frozen
proof strengths: **23 Exact, 5 Impossible, 10 Residual**. The immutable
child contracts remain authoritative. `tests/task_82_parent_acceptance.rs`
verifies identity and classification equality against all four active child
freeze files, stable fixture roots, the 13-dialect parser identity lists,
declared parser/adapter counts, original parent AC #1..#6 mappings,
and mutable coverage-manifest child signoff. This does **not** treat
parser success or an inventory count as native-engine execution.

Frozen historical signoff flags in TASK-153..156 are retained. Their
child task files and mutable coverage-manifest records reflect subsequent
approved completion. No freeze is rewritten to retroactively claim a
prospective implementation review.

## Parent AC #3 bounded MERGE idempotence (reviewed v3)

Two independently executable parent cases were added after preserving
`docs/task-82-parent-acceptance-freeze-v1.json` as historical evidence:

- `82-PARENT-IDEMPOTENT-P` starts with target [(1,10),(2,30)] and immutable
  source [(1,20),(3,40)]. The first MERGE updates key 1 and inserts key 3;
  the resulting bag is [(1,20),(2,30),(3,40)]. Reapplying the identical
  MERGE with that same source preserves the target *bag*, but it records
  **two matched UPDATE actions** (keys 1 and 3) on the second run.
- `82-PARENT-NONIDEMPOTENT-N` starts with target [(1,10)], source [(1,5)]
  and `SET score = t.score + s.score`. The first result is [(1,15)]
  and the second [(1,20)], proving this form is *not* idempotent.

Each step includes exact frozen pre/post bags, affected INSERT/UPDATE/DELETE
counts, typed INTEGER/NULL value frequencies, bounds, inclusivity and
nullability. `tests/task_82_parent_merge.rs` compares the public canonical
ordered plan with **independently executed DuckDB statements and full bags**,
including immutable source populations. The exact fixed-point result is
specific to its closed-world evidence, never an unconditional global
`WriteIdempotence::Proven` claim.

## Original parent AC evidence and disposition

1. **#1**: TASK-153/154 cover first-applicable matched and unmatched
   branches; TASK-155 classifies ON CONFLICT and vendor UPSERT restrictions;
   TASK-156 covers computed variants. Unsupported engine-specific
   equivalence remains explicitly Residual.
2. **#2**: TASK-153..156 prove source/target TRUE and NOT TRUE partitions,
   NULL nonmatches, repeated matches, keys, conflict and branch ordering.
   The parent mixed replay additionally checks both native partitions on
   each statement prestate.
3. **#3**: Child and parent source/target complete-bag and negative tests,
   plus the v3 positive fixed point and computed non-idempotence, prevent
   made-up final states for Impossible/Residual cases.
4. **#4**: TASK-154 three-way branch effects, TASK-155 constraint
   classifications, TASK-156 computed SET/INSERT and the parent two-step
   full-state replay jointly establish bounded MERGE composition.
5. **#5**: Complete DuckDB pre/action/post and sequential replays;
   child 13-dialect parser matrices and paired direct SQL/dbt/ODCS evidence.
   Parser acceptance is not native vendor engine proof.
6. **#6**: Strict format/lint, tests/docs with and without default
   features, dbt Core E2E, schema/golden checks, all child tests and
   parent verification tests execute in the nine-job CI matrix.
   No new public protocol shape, dependency or producer-specific heuristic
   has been introduced.

## Explicit conditional release boundaries

- **Duplicate source winners and native match-error laws:** TASK-155,
  Residual until versioned engine/cardinality evidence.
- **ON CONFLICT DO UPDATE, vendor UPSERT and MERGE OUTPUT:** TASK-155,
  do not promote to Exact or claim equivalent effect without pinned
  executed native oracle.
- **Nullable UNIQUE and foreign-key enforcement/deferral:** TASK-155,
  retain precise declared-enforcement and session-law Residual. A single
  DuckDB observation is not proof of universal vendor timing.
- **Unproved numeric expression overflow or type law:** TASK-156,
  fail closed as a typed Residual.
- **Transactions, implicit commit and isolation:** TASK-84 retains
  ownership; no such rule is fabricated from parser support.
- **Release:** TASK-91 independently reviews the remaining native
  boundaries before v3 publication. Parent closure never authorizes it.

These are conditional/native safety boundaries, **not deferred portable
mandatory Exact cases**. This distinction is necessary to avoid an
overclaim while completing the finite TASK-82 parent contract.

## Technical review / final gate

The earlier PR head `b05d4d83e0b31e6f7eec7401298b7cffc743ca4c`
passed [all nine GitHub CI jobs](https://github.com/phdah/sql-semantic-protocol/actions/runs/41922420700)
before the active v3 additions. Fresh final-head CI is mandatory.

Matt Standards/Spec author-account review is a technical assessment,
**not a distinct independent reviewer**. No child-specific review waiver
is inherited by TASK-82. Require a separate reviewer or an explicit
owner-directed process exception for parent signoff. Until that
disposition and final CI are verified, TASK-82 remains In Progress,
all six original AC stay unchecked and TASK-91 remains blocked.

## Reviewed parent v3 correction

The first CI execution of the new repeated-MERGE test rejected the v2
assumption that only one target row would be updated by the second statement.
The first MERGE inserted key 3, so **both key 1 and key 3 match** on the
second invocation. The correct independent DuckDB/canonical action oracle is
2 UPDATE actions, although the target *bag* remains a fixed point. The
incorrect v2 is preserved at `docs/task-82-parent-acceptance-freeze-v2.json`,
and v3 is the corrected active contract. This is a test-oracle correction,
not a change to the production MERGE interpreter or a silent alteration
of prior signoff evidence.

## Technical acceptance verification (2026-11-04)

The reviewed v3 implementation head `2f5b4c8157f37e2d018a5f99d6d93f53f7cb6ef0`
passed **all nine required GitHub Actions jobs**:
https://github.com/phdah/sql-semantic-protocol/actions/runs/41925632614 .
That includes fmt, default/no-default strict Clippy, both complete Rust test
configurations, both doc builds, dependency checks, active schema and
snapshot regressions, and actual dbt Core E2E. The Rust tests executed all
four parent-owned fixtures and all 38 frozen child cases via their own
public-API/native DuckDB, dialect and adapter tests. Both parent acceptance
inventory tests passed; all six original parent AC have a finite evidence
crosswalk and technically supported, conservative exactness boundaries.

**Final disposition (2026-11-05):** The owner explicitly approved this
parent technical audit and granted a TASK-82-specific exception to the
separate reviewer-identity gate in PR #186. This must not be described as an
independent-account GitHub review. All six original parent AC are now signed
off and TASK-82 is Done, with conditional boundaries retained. TASK-91 stays
separate. Final signoff-only tracking commits require green CI before merge.

## Owner approval and final merge gate (2026-11-05)

The author-account Matt `engineering/code-review` final technical review
found **0 remaining blocking Standards or Spec defects**. It strengthened
independent native DuckDB MERGE action-count verification via
`RETURNING merge_action`, preserved native-version-dependent duplicate-match
behavior as `Residual(UnprovedOperation)`, and rejected malformed frozen
INTEGER values. Final reviewed implementation head
`8cf1692c2adf51bfff13517c4c17281840ab14dc` passed
[all nine CI jobs](https://github.com/phdah/sql-semantic-protocol/actions/runs/41985740438)
including default/no-default Rust tests and dbt Core E2E.

The repository owner [approved PR #186 and expressly authorized a parent-specific
independent-review exception](https://github.com/phdah/sql-semantic-protocol/pull/186#issuecomment-6652370938).
This is **not** an independent GitHub reviewer identity and does not waive
any release gate outside TASK-82. The owner also accepted all documented
conditional vendor/constraint/session laws as typed Residual or Conditional;
no unsupported case was promoted to Exact. Original TASK-82 AC #1..#6 are
accepted and marked Done in the backlog and coverage manifest. Squash merge
of this PR is authorized after CI on the final tracking-only commit.
