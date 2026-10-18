---
id: TASK-114
title: Classify ASOF range time and LATERAL APPLY laws
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-10-18'
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

The versioned [TASK-114 acceptance matrix](../../docs/task-114-acceptance-freeze.json) enumerates twelve positive/NOT TRUE fixture cells across ASOF, range, time, LATERAL, CROSS APPLY and OUTER APPLY, with 156 dialect classification obligations. **All twelve canonical source-bag cases and the 156 dialect classifications are independently verified in CI; 154 parser-supported cells admit portable constructive proofs and two are documented parser boundaries. Native runtime/session laws retain five explicit conditional review records.**

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Freeze the two named fixture IDs and **every named variant above** into this child's versioned executable matrix: exact SQL/AST/dialect classification, source bag, stable source/column instance IDs, mandatory outcome domains (NULL and inclusive/exclusive bounds), positive/NOT TRUE proof, expected full output bag, adapter facts and explicitly owned conditional residuals; link the matrix from `docs/coverage-manifest.json`. Missing cells block Done.
- [x] #2 Implement the **finite named variants** in the canonical semantic/physical model. Match the stated independent oracle baseline exactly, including multiplicity, value-domain constraints and correct Exact/Impossible/Residual directions. No consumer SQL reparsing, invented physical rows, unknown-as-exact claims or mandatory variants silently relegated to Residual.
- [x] #3 Execute the baseline and a complementary rejected/NULL/duplicate or impossible case using complete DuckDB physical input and terminal-output bags. Assert individual row identities, counts, domain bounds and closed-world absence where applicable, not only a successful parser or a row count.
- [x] #4 For each supported shared form, classify all **13 exposed project dialects** as parser-equivalent or explicit parser boundary; check direct SQL and dbt plus ODCS when equivalent metadata exists. For each conditional vendor/session law, record specific missing evidence, stable typed residual/Impossible, owner TASK-114 and pending reviewed release disposition. No native engine certification without execution.
- [x] #5 Run focused public API tests, `cargo fmt --check`, strict Clippy, full tests, docs, no-default-features, dbt E2E and active protocol/schema snapshots on the final implementation head. Review actual assertions against each frozen fixture; only then mark this child Done and update its parent queue and coverage evidence. Green CI alone is insufficient.
<!-- AC:END -->

## Final acceptance verification (2026-10-18)

- **All five child AC signed off** after review of [PR #123](https://github.com/phdah/sql-semantic-protocol/pull/123), versioned [v7 12-case matrix](../../docs/task-114-acceptance-freeze.json) and [independent AC audit](../../docs/task-114-implementation-audit.md).
- AC #1: complete typed/nullable terminal domains, precise bounds, physical source identities, positive/NOT TRUE selected and rejected bags, 156 frozen parser statuses, six variants and adapter evidence. All 12 mandatory cells verified.
- AC #2: canonical nearest-prior ASOF and correlated row-scoped LATERAL/CROSS APPLY/OUTER APPLY finite source constructions; signed integer range and zone-free timestamp proofs; typed residuals for unsupported native session behavior and ASOF tie/absent-row preservation. No consumer SQL reparsing or invented physical rows.
- AC #3/#4: all 12 independent DuckDB source/result oracle bags; 154 constructive parser-supported dialect/case cells plus two explicit Snowflake OUTER APPLY parser boundaries; direct SQL, compiled dbt and equivalent ODCS source evidence parity. Five vendor-specific conditional laws have owner TASK-114 and remain pending native release review, not falsely certified.
- AC #5: [CI run 39113074116](https://github.com/phdah/sql-semantic-protocol/actions/runs/39113074116) is green for formatting, strict Clippy, full/default and no-default Rust tests, docs, dbt E2E and schema/protocol snapshots. The final signoff documentation commit must also pass CI before merge.
- **Merge gate:** marking TASK-114 Done records verified child acceptance, not a merged dependency. TASK-115 stays blocked until PR #123 merges to `main`; TASK-71 retains its independent parent signoff.

## Handoff

Next child: [TASK-115](task-115%20-%20compose-join-plans-through-filters-constraints-and-terminal-row-shaping.md). Do not begin the next child until this task is Done **and its PR merged**. Do not mark TASK-71 Done just because one child is complete.
