---
id: TASK-71
title: Construct exact join trees and dialect join variants
status: In Progress
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-17'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-109
  - TASK-110
  - TASK-111
  - TASK-112
  - TASK-113
  - TASK-114
  - TASK-115
  - TASK-116
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

## Implementation increment (2026-10-17)

- Initial finite TASK-71 inventory: [docs/task-71-acceptance-freeze.json](../../docs/task-71-acceptance-freeze.json), linked by `docs/coverage-manifest.json`. All mandatory and conditional cells remain unverified and block parent Done.
- Source population fix: filtered JOIN ON predicates may write equality operands in reverse order. The physical constructor now orients the recovered columns/source-instance identities by `Join::left()` and `Join::right()` before applying outer/anti/semi match and null-extension laws.
- Focused public API regression and independent DuckDB outer-join count oracle: `tests/physical_realization/joins.rs` (`filtered_outer_join_population_uses_sql_input_order_not_on_operand_order`).
- **Partial implementation only.** AC #1..#6 are still open. Mandatory chained join, composite key, all source/terminal oracle, adapter and dialect cases in the freeze are not certified by this increment. Review and explicitly verify all frozen cells before signing off TASK-71.

## Dependency-ordered bounded child queue (2026-10-17)

**This is a parent signoff task, not the next implementation task.** Pick the first unfinished child below whose dependencies are satisfied. Keep TASK-71 `In Progress` until all child PRs merge and original AC #1..#6 pass independently. Children must never depend on their parent; each next child depends on its immediate predecessor.

- **[TASK-109](task-109%20-%20prove-two-input-inner-join-bags-and-composite-keys.md)** (1/8): binary INNER equality; one-to-many key duplicates; two-column composite equality. Fixture IDs: `71-C109-P`, `71-C109-N`.
- **[TASK-110](task-110%20-%20compose-three-relation-chains-self-joins-and-shared-aliases.md)** (2/8): three-input INNER chain; self-join with independent aliases; repeated/shared physical producer. Fixture IDs: `71-C110-P`, `71-C110-N`.
- **[TASK-111](task-111%20-%20prove-left-right-and-full-unmatched-null-extension.md)** (3/8): LEFT/RIGHT/FULL matched and unmatched; SQL NULL and reversed-ON operands; physical vs synthetic NULL. Fixture IDs: `71-C111-P`, `71-C111-N`.
- **[TASK-112](task-112%20-%20certify-semi-and-anti-absence-witnesses.md)** (4/8): LEFT/RIGHT SEMI and ANTI; duplicates; SQL UNKNOWN; rejected nonmembership. Fixture IDs: `71-C112-P`, `71-C112-N`.
- **[TASK-113](task-113%20-%20implement-portable-join-syntax-and-boolean-condition-variants.md)** (5/8): CROSS; USING; NATURAL with known schema; OR-on; non-equi; IS NOT DISTINCT FROM; null-safe equality. Fixture IDs: `71-C113-P`, `71-C113-N`.
- **[TASK-114](task-114%20-%20classify-asof-range-time-and-lateral-apply-laws.md)** (6/8): ASOF nearest-prior; range/time inequalities; LATERAL; CROSS/OUTER APPLY; supported parser dialects. Fixture IDs: `71-C114-P`, `71-C114-N`.
- **[TASK-115](task-115%20-%20compose-join-plans-through-filters-constraints-and-terminal-row-shaping.md)** (7/8): two-hop WHERE then JOIN/JOIN; composite ON plus nullable keys; PK/FK; downstream GROUP/count. Fixture IDs: `71-C115-P`, `71-C115-N`.
- **[TASK-116](task-116%20-%20independently-certify-task-71-join-matrix-and-parent-acceptance.md)** (8/8): frozen J71 cells incl mandatory/conditional; SQL/dbt/ODCS; 13 supported dialect identities; DuckDB. Fixture IDs: `71-C116-P`, `71-C116-N`.

**Original AC to child ownership:** #1 -> TASK-109, TASK-110, TASK-111, TASK-112, TASK-113, TASK-115, TASK-116; #2 -> TASK-114, TASK-116; #3 -> TASK-109, TASK-110, TASK-111, TASK-112, TASK-113, TASK-114, TASK-115, TASK-116; #4 -> TASK-109, TASK-110, TASK-112, TASK-115, TASK-116; #5 -> TASK-109, TASK-110, TASK-111, TASK-112, TASK-113, TASK-114, TASK-115, TASK-116; #6 -> TASK-109, TASK-110, TASK-111, TASK-112, TASK-113, TASK-114, TASK-115, TASK-116.

**TASK-109 child evidence:** The first binary INNER / composite-key child has a bounded independent [TASK-109 acceptance audit](../../docs/task-109-acceptance-audit.md), signed in [PR #118](https://github.com/phdah/sql-semantic-protocol/pull/118). This certifies only the TASK-109 slice, not any entire parent AC. TASK-110 is not ready until that PR merges on `main`.

**Independent closure:** The parent keeps all original AC unchecked until every required frozen cell is executed, all conditional limits reviewed, adapter/13-dialect and DuckDB evidence reconciled, and final CI verified. The parent acceptance inventory is `docs/task-71-acceptance-freeze.json`, with the child execution plan at `docs/backlog-child-queue-plan.json`. No partial PR is a release approval.
