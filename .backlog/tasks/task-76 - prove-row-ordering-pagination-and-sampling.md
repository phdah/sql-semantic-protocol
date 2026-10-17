---
id: TASK-76
title: Handle row selection, ordering, pagination and sampling exactly
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
  - TASK-74
  - TASK-133
  - TASK-134
  - TASK-135
  - TASK-136
references: 
  - 'TASK-43'
  - 'TASK-64'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ORDER BY/LIMIT/OFFSET/FETCH/TOP, DISTINCT ON and sampling can invalidate seemingly exact row-membership and output counts.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Normalize LIMIT/OFFSET/FETCH WITH TIES/TOP, ORDER BY, DISTINCT/DISTINCT ON, QUALIFY placement and dialect-specific syntactic equivalents as typed row-selection semantics.
- [ ] #2 Derive exact candidate ranks, ties, offset windows and qualifying/rejected memberships with unique-order preconditions when necessary.
- [ ] #3 Describe deterministic versus stochastic TABLESAMPLE/SAMPLE with seed and sampling semantics; explicitly residual when exact output membership is inherently unprovable.
- [ ] #4 Compose inner/outer row limits across CTEs, set branches and producer layers, including empty and impossible output sizes.
- [ ] #5 Execute dialect-compatible full result tests under deterministic settings and add residual/no-overclaim cases.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-76 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: deterministic ORDER BY plus LIMIT/OFFSET/FETCH/TOP, WITH TIES and DISTINCT/DISTINCT ON where stable ordering is demonstrated.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: two nested pagination boundaries (CTE or set branch) with precise count and survivor/rejected outcomes, including empty input.
3. **Conditional and non-exact boundary:** Inventory conditional: TABLESAMPLE/SAMPLE, engine NULL/tie defaults and QUALIFY placement; stochastic or underordered membership remains residual with typed reasons.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Dependency-ordered bounded child queue (2026-10-17)

**TASK-76 is now a parent signoff, not one implementation PR.** Its original AC remain unchecked. Every child has its own finite variant/fixture IDs, complete source/terminal oracle and explicit conditional boundaries in `docs/backlog-child-queue-plan.json`. Select the first unfinished dependency-ready child; never add a parent dependency to a child. Do not sign off the parent merely because one PR passes CI.

- **[TASK-133](task-133%20-%20prove-deterministic-order-by-limit-offset-and-top.md)** (1/4): sorted total order; LIMIT/OFFSET; FETCH FIRST; TOP variant. Fixtures `76-C133-P`, `76-C133-N`.
- **[TASK-134](task-134%20-%20prove-ties-distinct-and-qualify-pagination-boundaries.md)** (2/4): WITH TIES; DISTINCT; DISTINCT ON; SQL NULL ordering. Fixtures `76-C134-P`, `76-C134-N`.
- **[TASK-135](task-135%20-%20compose-nested-page-boundaries-through-cte-and-sets.md)** (3/4): nested LIMIT+OFFSET CTE; set branch ORDER/LIMIT; producer copies. Fixtures `76-C135-P`, `76-C135-N`.
- **[TASK-136](task-136%20-%20classify-sampling-and-certify-task-76-dialect-oracle-matrix.md)** (4/4): seeded TABLESAMPLE/SAMPLE; dialect TOP/FETCH; NULL/tie laws; 13 dialects. Fixtures `76-C136-P`, `76-C136-N`.

**Original acceptance mapping:** #1 -> TASK-133, TASK-134, TASK-136; #2 -> TASK-133, TASK-134, TASK-135, TASK-136; #3 -> TASK-134, TASK-136; #4 -> TASK-135, TASK-136; #5 -> TASK-133, TASK-134, TASK-135, TASK-136; #6 -> TASK-133, TASK-134, TASK-135, TASK-136.

**Closure gate:** All child PRs merged, every original AC independently verified, every mandatory outcome/execution oracle green, conditional vendor/session cases owner-reviewed, adapter and 13-dialect matrix complete, final CI green. Neither parser coverage nor residual classification alone excuses a required exact variant. TASK-91 remains separately release-blocked.
