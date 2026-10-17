---
id: TASK-72
title: Prove full set-operation trees with aligned values and bags
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
  - TASK-117
  - TASK-118
  - TASK-119
  - TASK-120
references: 
  - 'TASK-58'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Existing set witness proof is narrow and optional cardinality construction handles just one-column two-branch physical sets.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Cover UNION/INTERSECT/EXCEPT DISTINCT and ALL, dialect MINUS, nested/chained multi-branch trees, multiple columns, NULL tuple equality, duplicates and compatible mixed types.
- [ ] #2 Model positional and dialect-supported CORRESPONDING/BY NAME alignment, aliases, coercions, overlapping/disjoint values, branch-local filters/joins/groups/limits and shared physical sources.
- [ ] #3 Prove both membership directions and exact tuple frequencies at physical sources through producer layers, not independent intermediate inserts.
- [ ] #4 Respect set-level and branch-level ORDER/LIMIT/FETCH and value/collation compatibility; downgrade only specific unsupported dialect variants.
- [ ] #5 Test executable entire tree results, impossible counts, NULLs, shared sources, multi-column histograms, and dbt set model fixtures.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-72 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), which are additive to the approved TASK-66 feature/dialect inventory. This section specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: two-column UNION, INTERSECT and EXCEPT, each DISTINCT and ALL, with duplicate counts and SQL NULL tuple equality.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: three-branch nested tree, positional type/alias alignment and at least one shared-source branch/filter with complete physical row obligations.
3. **Conditional and non-exact boundary:** Inventory conditional: MINUS, BY NAME/CORRESPONDING, coercion/collation, branch/set ORDER/LIMIT and nontrivial join/group branches; exact only with proven tuple histograms.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Each mandatory core cell needs independently asserted typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, full DuckDB source/terminal oracle when executable, and representable SQL/dbt/ODCS adapter plus 13-dialect parser equivalence coverage. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after all required cells and final CI are verified. Do not promote this child to release approval or silently amend parent acceptance criteria.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Dependency-ordered bounded child queue (2026-10-17)

**TASK-72 is now a parent signoff, not one implementation PR.** Its original AC remain unchecked. Every child has its own finite variant/fixture IDs, complete source/terminal oracle and explicit conditional boundaries in `docs/backlog-child-queue-plan.json`. Select the first unfinished dependency-ready child; never add a parent dependency to a child. Do not sign off the parent merely because one PR passes CI.

- **[TASK-117](task-117%20-%20prove-two-column-set-bag-and-null-laws.md)** (1/4): UNION/INTERSECT/EXCEPT each DISTINCT/ALL; tuple duplicates and NULL equality. Fixtures `72-C117-P`, `72-C117-N`.
- **[TASK-118](task-118%20-%20compose-nested-branches-and-shared-physical-sources.md)** (2/4): three-branch nested sets; branch-local WHERE; shared physical input; produced CTE/model. Fixtures `72-C118-P`, `72-C118-N`.
- **[TASK-119](task-119%20-%20resolve-alignment-limits-and-vendor-set-forms.md)** (3/4): positional aliases/coercion; BY NAME/CORRESPONDING; MINUS; local and set ORDER/LIMIT/FETCH. Fixtures `72-C119-P`, `72-C119-N`.
- **[TASK-120](task-120%20-%20certify-task-72-adapter-and-dialect-set-matrix.md)** (4/4): mandatory set family matrix; NULL tuple and multiset oracles; sql/dbt/ODCS; 13 dialects. Fixtures `72-C120-P`, `72-C120-N`.

**Original acceptance mapping:** #1 -> TASK-117, TASK-118, TASK-119, TASK-120; #2 -> TASK-118, TASK-119, TASK-120; #3 -> TASK-117, TASK-118, TASK-120; #4 -> TASK-119, TASK-120; #5 -> TASK-117, TASK-118, TASK-119, TASK-120; #6 -> TASK-117, TASK-118, TASK-119, TASK-120.

**Closure gate:** All child PRs merged, every original AC independently verified, every mandatory outcome/execution oracle green, conditional vendor/session cases owner-reviewed, adapter and 13-dialect matrix complete, final CI green. Neither parser coverage nor residual classification alone excuses a required exact variant. TASK-91 remains separately release-blocked.
