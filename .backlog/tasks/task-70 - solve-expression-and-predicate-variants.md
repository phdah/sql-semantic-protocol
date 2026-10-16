---
id: TASK-70
title: Construct typed correlated predicate and expression outcomes
status: In Progress
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-13'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
  - TASK-79
  - TASK-101
  - TASK-102
  - TASK-103
  - TASK-104
  - TASK-105
  - TASK-106
  - TASK-107
  - TASK-108
references: 
  - 'TASK-46'
  - 'TASK-63'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Current boolean proof supports a restricted scalar subset. Expand safe, typed expression constraints without requiring sql-tdg to infer SQL semantics.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Cover arithmetic and overflow, unary/binary comparisons, BETWEEN/IN lists, IS NULL / IS DISTINCT FROM, NULL-safe equality, CASE, COALESCE, NULLIF, supported CAST/TRY_CAST, string LIKE/NOT LIKE/ESCAPE and expressions in join, filter, grouping and assignments.
- [ ] #2 Support mixed AND/OR/NOT, cross-column comparisons, computed projections and aliases with SQL three-valued logic and reversible typed constraints.
- [ ] #3 Catalog which dialect-sensitive regex/pattern, collations, implicit casts, date/time, JSON or vendor function families have safe exact semantics; opaque UDFs must retain explicit residuals.
- [ ] #4 Give canonical positive and NOT TRUE negative witnesses, impossible cases, output domains, comparison settings and deterministic literal coercion.
- [ ] #5 Add direct SQL, dbt and representative per-dialect parser/oracle tests for each supported expression family and combined predicates.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#6 remain required**. Interpret and verify them through the frozen TASK-70 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), which are additive to the approved TASK-66 feature/dialect inventory. This section specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Parent sign-off: TASK-101..107 and TASK-108 map the six original AC, with each family in the approved TASK-66 inventory classified.
2. **Mandatory cross-operator / adapter evidence:** Finite child matrix: signed arithmetic/casts, CASE/COALESCE/NULLIF, correlated predicates, text patterns, temporal/JSON/vendor boundaries, composed physical graphs, SQL/dbt/13-dialect oracle evidence.
3. **Conditional and non-exact boundary:** TASK-108 must audit every original AC and unresolved family; no parent completion from bounded TASK-106 alone.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Each mandatory core cell needs independently asserted typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, full DuckDB source/terminal oracle when executable, and representable SQL/dbt/ODCS adapter plus 13-dialect parser equivalence coverage. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#6 only after all required cells and final CI are verified. Do not promote this child to release approval or silently amend parent acceptance criteria.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## TASK-70 parent and ordered child implementation queue (2026-10-13)

**PR #108 is a mergeable foundational increment, not TASK-70 signoff.** It introduces a bounded typed three-valued witness subset and audited fail-closed evidence. Once PR #108 is merged into `main`, do not reimplement the parent as a large PR. The parent remains **In Progress**, its six original criteria **unchecked**, and TASK-91/protocol 3.0.0 **blocked** until all children and closure evidence are verified.

**Execution order:** Pick the **first unfinished child with satisfied dependencies** in this queue before unrelated backlog tasks when asked for the "next task". Start with **TASK-101** after PR #108 merges. Every child gets its own focused PR **against `main`**; merge and mark the child Done only after all its acceptance criteria and CI are verified. Do not skip an incomplete child. The parent depends on its children for *closure*, but children never depend on TASK-70 (avoids cycles).

- **[TASK-101](task-101%20-%20prove-overflow-safe-arithmetic-and-typed-conversions.md)**: Prove typed arithmetic, overflow and conversion outcomes
- **[TASK-102](task-102%20-%20normalize-case-coalesce-nullif-and-computed-aliases.md)**: Prove CASE, COALESCE, NULLIF and computed alias outcomes
- **[TASK-103](task-103%20-%20prove-correlated-cross-column-predicate-constraints.md)**: Prove correlated cross-column and mixed Boolean constraints
- **[TASK-104](task-104%20-%20certify-pattern-collation-and-nullsafe-expression-laws.md)**: Certify pattern, collation and null-safe expression laws
- **[TASK-105](task-105%20-%20classify-temporal-json-and-vendor-expression-semantics.md)**: Classify temporal, JSON and vendor expression outcomes
- **[TASK-106](task-106%20-%20compose-computed-predicates-across-relational-graphs.md)**: Compose typed expression obligations through relational graphs
- **[TASK-107](task-107%20-%20certify-expression-predicate-dialect-dbt-oracles.md)**: Certify expression and predicate SQL/dbt/dialect oracles
- **[TASK-108](task-108%20-%20signoff-task70-typed-predicate-expression-outcomes.md)**: Sign off TASK-70 typed predicate and expression outcomes

**Original acceptance mapping:** #1: TASK-101/102/104/106; #2: TASK-102/103/106; #3: TASK-104/105; #4: TASK-101/103/106; #5: TASK-107; #6: TASK-107/108. TASK-108 checks these mappings against actual evidence and signs off the original six criteria. Cross-feature TASK-88 (dialect), TASK-89 (integrated E2E), TASK-90 (v3 contract), and TASK-91 (release) keep their independent scopes and gates; no consumer-side SQL semantics are added.

## Implementation progress (2026-10-13)

The first bounded canonical Boolean-predicate increment lives on
`feat/task-70-typed-predicate-witnesses`:

- Typed integer `BETWEEN` / `NOT BETWEEN` and finite `IN` / `NOT IN` reuse the existing scalar comparison witness IR.
- `NOT` normalizes with De Morgan's laws while preserving SQL UNKNOWN;
  catalog-typed `IS [NOT] DISTINCT FROM` integers and direct `NULL`
  literals have exact positive and NOT TRUE witnesses.
- Exact conjunctive NULL tests now refine source and projected domains.
- SQL three-valued DuckDB oracles, parser-dialect coverage, and
  unsupported computed-expression residual tests are added.
- Ordinary literal-NULL comparisons preserve UNKNOWN, including finite
  NULL-bearing IN lists, with impossible TRUE classifications when proven.
- Total signed integer identity casts/offsets and catalog-typed COALESCE/NULLIF
  NULL tests now have canonical, two-valued row truth witnesses.
- Searched CASE of total signed integer columns with two-valued guards now
  supports NULL-test outcome selection without collapsing SQL UNKNOWN.
- Safe LIKE prefixes accept an inert explicit single-character ESCAPE only
  under the existing binary-collation and no-padding proof requirements.
- Catalog-typed integer self-comparisons now prove impossible or tautological
  outcomes exhaustively, without extending that claim to unrelated columns.
- Formatting-free total signed-integer TRY_CAST and SAFE_CAST share the existing
  catalog-certified inverse path; narrowing and untyped conversions stay residual.
- No consumer-specific SQL semantics or parser AST in protocol types.
  The protocol schema and emitted wire contract now include a canonical
  `unknown_null_comparison` constraint, and dbt snapshots are refreshed.

**Outstanding (do not sign off):** General arithmetic and overflow,
computed projections/aliases and CASE/COALESCE/NULLIF beyond the certified
NULL-test subset; noninteger conversions, narrowing and implicit coercion;
nontrivial multi-column impossibility, advanced pattern matching,
dialect-sensitive functions/session laws, general join/group/write computed
expression proofs, complete physical populations, and exhaustive 13-dialect,
dbt and engine oracles. The machine-readable per-family scope is now in
`docs/coverage-manifest.json.task_70_predicate_evidence.family_evidence`.
All six AC remain open in their full declared scope and are now owned by the ordered TASK-101..108 children; TASK-91 stays blocked.
