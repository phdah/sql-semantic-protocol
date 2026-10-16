---
id: TASK-71
title: Construct exact join trees and dialect join variants
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
references: 
  - 'TASK-61'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
One-binary-join proofs cannot generate exact datasets for realistic multi-join, self-join and outer-join pipelines.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Cover INNER/LEFT/RIGHT/FULL/CROSS, SEMI/ANTI, SELF, USING/NATURAL, composite ON, OR/non-equi/null-safe conditions, mixed and chained join trees, alias reuse and many-to-many multiplicities.
- [ ] #2 Evaluate ASOF/range/time joins and LATERAL/APPLY constructs for each parser-supported dialect, translating supported semantics into the same typed plan.
- [ ] #3 Construct matched, unmatched, NULL-extended and rejected rows with explicit source instance identity and complete absence obligations; never treat null-extension as inserted physical NULL rows.
- [ ] #4 Compose upstream filters and downstream row-shaping, keys/FKs, nullable join columns and duplicate matches while satisfying output cardinality.
- [ ] #5 Verify exact output row identities/counts in DuckDB and dialect engines where available; unsupported engine semantics retain traceable residuals.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-71 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), which are additive to the approved TASK-66 feature/dialect inventory. This section specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: two- and three-relation INNER equijoins (including one-to-many duplicate keys); LEFT/RIGHT/FULL unmatched NULL-extension; SEMI/ANTI absence; self-join with two aliases.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: two-hop join with upstream WHERE and output count, composite equality keys and explicit NULL/rejection examples, all from one closed physical input bag.
3. **Conditional and non-exact boundary:** Inventory conditional: CROSS, USING/NATURAL, OR/non-equi/null-safe, ASOF/time/range and LATERAL/APPLY by dialect; evidence-backed exact cell or typed residual with owner and release disposition.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Each mandatory core cell needs independently asserted typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, full DuckDB source/terminal oracle when executable, and representable SQL/dbt/ODCS adapter plus 13-dialect parser equivalence coverage. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after all required cells and final CI are verified. Do not promote this child to release approval or silently amend parent acceptance criteria.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.
