---
id: TASK-116
title: Independently certify TASK-71 join matrix and parent acceptance
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-18'
labels: []
milestone: m-3
dependencies:
  - TASK-115
references:
  - TASK-71
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-71 child 8/8.** Independently certify TASK-71 join matrix and parent acceptance. Work in a separate focused PR against `main` after its listed dependencies are Done and merged. The original TASK-71 acceptance criteria remain mandatory for parent signoff. This child is independent of its parent as a dependency; it must not depend on TASK-71.
<!-- SECTION:DESCRIPTION:END -->

## Frozen planning scope

- **Stable acceptance fixture IDs:** `71-C116-P`, `71-C116-N`. Register them as immutable, executable versioned fixtures in the task-specific coverage matrix **before changing semantics**.
- **Named feature/variant inventory (finite):** frozen J71 cells incl mandatory/conditional; SQL/dbt/ODCS; 13 supported dialect identities; DuckDB.
- **Concrete independent oracle baseline:** Verify all 15 mandatory, 12 conditional inventory cells, each original AC #1..#6 and full CI; zero unchecked mandatory cells.
- **Conditional/fail-closed boundary:** Do not promote conditional parser only claims to native engine certification; parent only Done after cross-case signoff.
- **Original parent AC ownership:** #1, #2, #3, #4, #5, #6. Additive to project-wide outcome-first definition of done and TASK-66 capability inventory.

## Implementation evidence (2026-10-18)

- Independent [TASK-116 versioned freeze](../../docs/task-116-acceptance-freeze.json) and [parent oracle](../../tests/task_116_parent_join_oracles.rs) cover 15 mandatory parent bags and 12 conditional source-law classifications, with independent SQL row identities, typed domains and closed-world NOT TRUE cases.
- [Parent reconciliation audit](../../docs/task-116-acceptance-audit.md) preserves original TASK-71 signoff gates. Final-head CI verified in [run 39155624537](https://github.com/phdah/sql-semantic-protocol/actions/runs/39155624537) with all nine checks passing. All five bounded TASK-116 criteria are verified by independent parent bag tests and child SQL/dbt/ODCS plus dialect evidence. Native vendor/session runtime is explicitly uncertified; no portable parser claim is promoted to native proof. TASK-71 remains In Progress until this PR merges and parent closure is recorded.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Freeze the two named fixture IDs and **every named variant above** into this child's versioned executable matrix: exact SQL/AST/dialect classification, source bag, stable source/column instance IDs, mandatory outcome domains (NULL and inclusive/exclusive bounds), positive/NOT TRUE proof, expected full output bag, adapter facts and explicitly owned conditional residuals; link the matrix from `docs/coverage-manifest.json`. Missing cells block Done.
- [x] #2 Implement the **finite named variants** in the canonical semantic/physical model. Match the stated independent oracle baseline exactly, including multiplicity, value-domain constraints and correct Exact/Impossible/Residual directions. No consumer SQL reparsing, invented physical rows, unknown-as-exact claims or mandatory variants silently relegated to Residual.
- [x] #3 Execute the baseline and a complementary rejected/NULL/duplicate or impossible case using complete DuckDB physical input and terminal-output bags. Assert individual row identities, counts, domain bounds and closed-world absence where applicable, not only a successful parser or a row count.
- [x] #4 For each supported shared form, classify all **13 exposed project dialects** as parser-equivalent or explicit parser boundary; check direct SQL and dbt plus ODCS when equivalent metadata exists. For each conditional vendor/session law, record specific missing evidence, stable typed residual/Impossible, owner TASK-116 and pending reviewed release disposition. No native engine certification without execution.
- [x] #5 Run focused public API tests, `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and active protocol/schema snapshots on the final implementation head. Review actual assertions against each frozen fixture; only then mark this child Done and update its parent queue and coverage evidence. Green CI alone is insufficient.
<!-- AC:END -->

## Handoff

Next child: independent TASK-71 parent AC signoff. Do not begin the next child until this task is Done **and its PR merged**. Do not mark TASK-71 Done just because one child is complete.
