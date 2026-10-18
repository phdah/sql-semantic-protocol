---
id: TASK-115
title: Compose join plans through filters constraints and terminal row shaping
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-114
references:
  - TASK-71
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-71 child 7/8.** Compose join plans through filters constraints and terminal row shaping. Work in a separate focused PR against `main` after its listed dependencies are Done and merged. The original TASK-71 acceptance criteria remain mandatory for parent signoff. This child is independent of its parent as a dependency; it must not depend on TASK-71.
<!-- SECTION:DESCRIPTION:END -->

## Frozen planning scope

- **Stable acceptance fixture IDs:** `71-C115-P`, `71-C115-N`. Register them as immutable, executable versioned fixtures in the task-specific coverage matrix **before changing semantics**.
- **Named feature/variant inventory (finite):** two-hop WHERE then JOIN/JOIN; composite ON plus nullable keys; PK/FK; downstream GROUP/count.
- **Concrete independent oracle baseline:** L keys [1,1,2] WHERE a>0, R keys [1,1], T keys [1] => positive joined count 4; rejecting NULL key excluded.
- **Conditional/fail-closed boundary:** Must prove same complete physical input bag for upstream filters, all join instances and terminal count.
- **Original parent AC ownership:** #1, #3, #4, #5, #6. Additive to project-wide outcome-first definition of done and TASK-66 capability inventory.

## Verified implementation evidence

The [versioned eight-case source-bag matrix](../../docs/task-115-acceptance-freeze.json)
records exact source and column instances, nullable typed value bounds,
complete physical TRUE/NOT TRUE Cartesian partitions, eight frozen P/N
terminal bags, 104 canonical dialect obligations and 24 cross-adapter cells.
The [independent acceptance audit](../../docs/task-115-acceptance-audit.md)
links public physical proof and DuckDB oracle tests plus dbt and ODCS
adapter parity. PK/FK proof requires explicit authoritative enforcement and
source text comparison evidence, never metadata-only assumptions.
The conditional native-vendor/session claims are reviewed but remain
uncertified and are not represented as portable semantics. All five bounded
child criteria are verified. TASK-71 remains In Progress until TASK-116
independently certifies the parent matrix.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Freeze the two named fixture IDs and **every named variant above** into this child's versioned executable matrix: exact SQL/AST/dialect classification, source bag, stable source/column instance IDs, mandatory outcome domains (NULL and inclusive/exclusive bounds), positive/NOT TRUE proof, expected full output bag, adapter facts and explicitly owned conditional residuals; link the matrix from `docs/coverage-manifest.json`. Missing cells block Done.
- [x] #2 Implement the **finite named variants** in the canonical semantic/physical model. Match the stated independent oracle baseline exactly, including multiplicity, value-domain constraints and correct Exact/Impossible/Residual directions. No consumer SQL reparsing, invented physical rows, unknown-as-exact claims or mandatory variants silently relegated to Residual.
- [x] #3 Execute the baseline and a complementary rejected/NULL/duplicate or impossible case using complete DuckDB physical input and terminal-output bags. Assert individual row identities, counts, domain bounds and closed-world absence where applicable, not only a successful parser or a row count.
- [x] #4 For each supported shared form, classify all **13 exposed project dialects** as parser-equivalent or explicit parser boundary; check direct SQL and dbt plus ODCS when equivalent metadata exists. For each conditional vendor/session law, record specific missing evidence, stable typed residual/Impossible, owner TASK-115 and pending reviewed release disposition. No native engine certification without execution.
- [x] #5 Run focused public API tests, `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and active protocol/schema snapshots on the final implementation head. Review actual assertions against each frozen fixture; only then mark this child Done and update its parent queue and coverage evidence. Green CI alone is insufficient.
<!-- AC:END -->

## Handoff

Next child: [TASK-116](task-116%20-%20independently-certify-task-71-join-matrix-and-parent-acceptance.md). Do not begin the next child until this task is Done **and its PR merged**. Do not mark TASK-71 Done just because one child is complete.
