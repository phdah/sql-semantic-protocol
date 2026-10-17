---
id: TASK-109
title: Prove two-input inner join bags and composite keys
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
references:
  - TASK-71
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-71 child 1/8.** Prove two-input inner join bags and composite keys. Work in a separate focused PR against `main` after its listed dependencies are Done and merged. The original TASK-71 acceptance criteria remain mandatory for parent signoff. This child is independent of its parent as a dependency; it must not depend on TASK-71.
<!-- SECTION:DESCRIPTION:END -->

## Frozen planning scope

- **Stable acceptance fixture IDs:** `71-C109-P`, `71-C109-N`. Register them as immutable, executable versioned fixtures in the task-specific coverage matrix **before changing semantics**.
- **Named feature/variant inventory (finite):** binary INNER equality; one-to-many key duplicates; two-column composite equality.
- **Concrete independent oracle baseline:** L=[(1,10),(1,20),(NULL,30)], R=[(1,10),(1,50),(NULL,70)]; simple ON k: 4 matched rows, composite ON (k,a): 1 row; NULL join keys never match.
- **Conditional/fail-closed boundary:** No full chained topology; unsupported expressions stay typed residual.
- **Original parent AC ownership:** #1, #3, #4, #5, #6. Additive to project-wide outcome-first definition of done and TASK-66 capability inventory.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Freeze the two named fixture IDs and **every named variant above** into this child's versioned executable matrix: exact SQL/AST/dialect classification, source bag, stable source/column instance IDs, mandatory outcome domains (NULL and inclusive/exclusive bounds), positive/NOT TRUE proof, expected full output bag, adapter facts and explicitly owned conditional residuals; link the matrix from `docs/coverage-manifest.json`. Missing cells block Done.
- [x] #2 Implement the **finite named variants** in the canonical semantic/physical model. Match the stated independent oracle baseline exactly, including multiplicity, value-domain constraints and correct Exact/Impossible/Residual directions. No consumer SQL reparsing, invented physical rows, unknown-as-exact claims or mandatory variants silently relegated to Residual.
- [x] #3 Execute the baseline and a complementary rejected/NULL/duplicate or impossible case using complete DuckDB physical input and terminal-output bags. Assert individual row identities, counts, domain bounds and closed-world absence where applicable, not only a successful parser or a row count.
- [x] #4 For each supported shared form, classify all **13 exposed project dialects** as parser-equivalent or explicit parser boundary; check direct SQL and dbt plus ODCS when equivalent metadata exists. For each conditional vendor/session law, record specific missing evidence, stable typed residual/Impossible, owner TASK-109 and pending reviewed release disposition. No native engine certification without execution.
- [x] #5 Run focused public API tests, `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and active protocol/schema snapshots on the final implementation head. Review actual assertions against each frozen fixture; only then mark this child Done and update its parent queue and coverage evidence. Green CI alone is insufficient.
<!-- AC:END -->

## Frozen executable inventory

The child-specific v1 acceptance inventory is [docs/task-109-acceptance-freeze.json](../../docs/task-109-acceptance-freeze.json), linked from the canonical coverage manifest. Exact fixtures name every positive pair, NOT TRUE pair, source instance and expected output-domain bound, as well as dialect, adapter and final CI gates. The public regression target is `tests/task_109_composite_join.rs`; `tests/task_109_acceptance_freeze.rs` checks inventory closure.

The physical INNER-join constructor interprets each extra AND-conjoined equality as part of the **same complete key-group assignment**, not as an independent join pair. The canonical `join_population.additional_keys` property is emitted only for composite keys. No status or release claim is certified by the frozen inventory alone.

## Verified acceptance signoff (2026-10-17)

All five original child ACs are checked against the [independent task-specific audit](../../docs/task-109-acceptance-audit.md). The review ran every frozen mandatory query on complete DuckDB input/output bags, compared exact source row IDs and SQL TRUE/NOT TRUE partitions, verified the canonical two-key INNER proof, the 13-dialect parser/canonical equivalence and compiled dbt/ODCS paths, and retained four individually reviewed conservative boundaries. The v2 executable acceptance matrix retains the original five variant IDs and expected row counts. Nine CI checks passed on implementation head `e4ce883e` ([run 39025591446](https://github.com/phdah/sql-semantic-protocol/actions/runs/39025591446)); the final PR-head CI is the merge gate.

This child is Done **on this PR branch**. It is not a release approval; TASK-71 remains In Progress. The next child TASK-110 is not eligible until PR #118 has actually merged.

## Handoff

Next child: [TASK-110](task-110%20-%20compose-three-relation-chains-self-joins-and-shared-aliases.md). Do not begin the next child until this task is Done **and its PR merged**. Do not mark TASK-71 Done just because one child is complete.
