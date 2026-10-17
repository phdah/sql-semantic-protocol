---
id: TASK-114
title: Classify ASOF range time and LATERAL APPLY laws
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies:
  - TASK-113
references:
  - TASK-71
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-71 child 6/8.** Classify ASOF range time and LATERAL APPLY laws. Work in a separate focused PR against `main` after its listed dependencies are Done and merged. The original TASK-71 acceptance criteria remain mandatory for parent signoff. This child is independent of its parent as a dependency; it must not depend on TASK-71.
<!-- SECTION:DESCRIPTION:END -->

## Frozen planning scope

- **Stable acceptance fixture IDs:** `71-C114-P`, `71-C114-N`. Register them as immutable, executable versioned fixtures in the task-specific coverage matrix **before changing semantics**.
- **Named feature/variant inventory (finite):** ASOF nearest-prior; range/time inequalities; LATERAL; CROSS/OUTER APPLY; supported parser dialects.
- **Concrete independent oracle baseline:** L times [5,10], R times [4,7] => nearest-prior ASOF picks [4,7]; correlated LATERAL two singletons => 2 rows.
- **Conditional/fail-closed boundary:** Vendor/session tie-breaking and per-row table-function side effects must fail closed with named conditional owner.
- **Original parent AC ownership:** #2, #3, #5, #6. Additive to project-wide outcome-first definition of done and TASK-66 capability inventory.

## Frozen acceptance inventory

The versioned [TASK-114 acceptance matrix](../../docs/task-114-acceptance-freeze.json) enumerates twelve positive/NOT TRUE fixture cells across ASOF, range, time, LATERAL, CROSS APPLY and OUTER APPLY, with 156 dialect classification obligations. **Every cell is still unverified; this file freezes scope, not evidence or signoff.**

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Freeze the two named fixture IDs and **every named variant above** into this child's versioned executable matrix: exact SQL/AST/dialect classification, source bag, stable source/column instance IDs, mandatory outcome domains (NULL and inclusive/exclusive bounds), positive/NOT TRUE proof, expected full output bag, adapter facts and explicitly owned conditional residuals; link the matrix from `docs/coverage-manifest.json`. Missing cells block Done.
- [ ] #2 Implement the **finite named variants** in the canonical semantic/physical model. Match the stated independent oracle baseline exactly, including multiplicity, value-domain constraints and correct Exact/Impossible/Residual directions. No consumer SQL reparsing, invented physical rows, unknown-as-exact claims or mandatory variants silently relegated to Residual.
- [x] #3 Execute the baseline and a complementary rejected/NULL/duplicate or impossible case using complete DuckDB physical input and terminal-output bags. Assert individual row identities, counts, domain bounds and closed-world absence where applicable, not only a successful parser or a row count.
- [ ] #4 For each supported shared form, classify all **13 exposed project dialects** as parser-equivalent or explicit parser boundary; check direct SQL and dbt plus ODCS when equivalent metadata exists. For each conditional vendor/session law, record specific missing evidence, stable typed residual/Impossible, owner TASK-114 and pending reviewed release disposition. No native engine certification without execution.
- [ ] #5 Run focused public API tests, `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and active protocol/schema snapshots on the final implementation head. Review actual assertions against each frozen fixture; only then mark this child Done and update its parent queue and coverage evidence. Green CI alone is insufficient.
<!-- AC:END -->

## Implementation verification iteration (2026-10-17)

- Draft delivery: [PR #123](https://github.com/phdah/sql-semantic-protocol/pull/123), with evidence and **unmet original AC** audited in [docs/task-114-implementation-audit.md](../../docs/task-114-implementation-audit.md).
- Frozen v5 matrix now records 12 typed P/N SQL/DuckDB bags, explicit output domains, full physical source identities and 156 literal dialect-parser classifications (52 shared inequality ASTs, 102 parsed-but-semantically-residual cells, two parser boundaries).
- Exact signed-integer comparisons `<`, `>`, `<=` and `>=` have complete closed-world physical source plans, inclusive/exclusive comparisons, reversed operands and direct/dbt/ODCS parity. Independent DuckDB oracles assert complete ASOF/LATERAL/APPLY terminal and nonselected bags, including timestamp equality and NULL/duplicates.
- Canonical ASOF eligibility is represented as a typed predicate while nearest-row selection remains explicitly unproven; LATERAL and APPLY are reported as residual where correlated populations cannot be constructed.
- **Only child AC #3 (independent DuckDB source/terminal oracle) is signed off by CI run 39073582470; AC #1, #2, #4 and #5 are not. Do not sign off this task, mark Done or advance to TASK-115.** ASOF nearest-prior and per-left-row LATERAL/APPLY *canonical constructive* proof laws, full vendor/runtime/adaptor semantic evidence and final fixture-level verification remain mandatory. Green CI alone is insufficient.

## Handoff

Next child: [TASK-115](task-115%20-%20compose-join-plans-through-filters-constraints-and-terminal-row-shaping.md). Do not begin the next child until this task is Done **and its PR merged**. Do not mark TASK-71 Done just because one child is complete.
