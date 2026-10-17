---
id: TASK-86
title: Define exact multi-terminal qualifying and rejected witness contracts
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
  - TASK-85
  - TASK-169
  - TASK-170
  - TASK-171
  - TASK-172
references: 
  - 'sql-tdg TASK-35'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Whole-project dbt generation must preserve all terminals while producing deliberate per-outcome negatives. The upstream contract must express the chosen semantics, not let downstream infer rejection.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Coordinate with sql-tdg TASK-35: record maintainer-approved meaning of rejected source rows across terminals; do not choose semantics implicitly.
- [ ] #2 Represent per-source-row membership vector for each terminal outcome with positive, FALSE/UNKNOWN nonmembership, proven absence, shared lineage and counterfactual membership where relevant.
- [ ] #3 Prove jointly feasible positive/negative rows for shared and disjoint sources, conflicting terminal filters, grouping and row-shaping, maintaining all enforced constraints.
- [ ] #4 Expose per-terminal provenance and stable outcome identities and independently classify impossible vs residual negative directions.
- [ ] #5 Add executable full-db-project fixture tests showing exact presence/absence in every targeted terminal output.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
- [ ] #7 Enumerate all provably constructive per-terminal rejecting alternatives with typed predicate/column identities (including composed Boolean expressions), independent FALSE/UNKNOWN/absence proofs and provenance; the generator may choose among them without SQL reparsing.
- [ ] #8 Specify a deterministic seeded-randomized selection contract that can reach every feasible alternative across seeds; demonstrate different rejected columns, preserve shared constraints, and distinguish globally impossible versus residual alternatives.
- [ ] #9 Counterexamples such as falsifying one arm of A OR B while B passes must **not** be considered rejected; prove nonmembership of each selected terminal after all alternative branches/lineage paths.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#9 remain required**. Interpret and verify them through the frozen TASK-86 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Exact core: each source row has per-terminal positive, FALSE, UNKNOWN or absence membership with stable lineage; A OR B rejection requires both arms fail.
2. **Mandatory cross-operator / adapter evidence:** Exact composition: two simultaneous terminal filters over shared inputs with at least two distinct feasible rejection columns, seeded deterministic reachability and complete DuckDB terminal snapshots.
3. **Conditional and non-exact boundary:** Inventory conditional: grouped/window/set rejected alternatives and counterfactuals; record all provably constructive alternatives for frozen fixture predicates, not arbitrary formulas.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#9 only after required cells and final CI are verified. Do not approve independent release gates through this task.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Maintainer decision (2026-10-09)

Per-terminal rejected-row classification and **seeded randomized rejecting predicate/column selection** are approved in [docs/coverage-signoff.md](../../docs/coverage-signoff.md). The protocol owns enumerated constructive alternatives and proof; sql-tdg TASK-35 owns unbiased or explicitly disclosed sampling over eligible choices using its injected RNG. Approval completes decision criterion #1 only; no implementation or E2E acceptance is implied.

## Bounded dependency-ordered child execution queue (2026-10-17)

TASK-86 is an **independent parent signoff** and cannot be closed from a narrow passing CI subset. All original AC remain mandatory and unchecked until every child below is Done and merged. Choose first unfinished child with satisfied upstream dependencies; no child depends on TASK-86. `docs/backlog-child-queue-plan.json` is the versioned planning inventory, and each child must register a frozen, executable task-specific acceptance matrix before semantics change.

- **[TASK-169](task-169%20-%20represent-per-source-per-terminal-true-false-unknown-and-absence-vectors.md)** (1/4): per-terminal membership; column identities; shared lineage; OR rejection semantics. Fixtures `86-C169-P`, `86-C169-N`.
- **[TASK-170](task-170%20-%20enumerate-all-constructive-rejection-alternatives-with-provenance.md)** (2/4): multiple AND/OR choices; independent FALSE/UNKNOWN; column-specific branches. Fixtures `86-C170-P`, `86-C170-N`.
- **[TASK-171](task-171%20-%20specify-seeded-reachability-and-global-compatibility.md)** (3/4): stable deterministic RNG seed selection; every feasible rejection alternative; shared constraints. Fixtures `86-C171-P`, `86-C171-N`.
- **[TASK-172](task-172%20-%20compose-project-wide-positive-negative-snapshots-and-certify-task-86.md)** (4/4): two terminals; grouped/window/set negatives; sql-tdg TASK-35 semantics. Fixtures `86-C172-P`, `86-C172-N`.

**Original AC ownership:** #1 -> TASK-172; #2 -> TASK-169, TASK-170, TASK-172; #3 -> TASK-171, TASK-172; #4 -> TASK-169, TASK-170, TASK-172; #5 -> TASK-169, TASK-170, TASK-171, TASK-172; #6 -> TASK-169, TASK-170, TASK-171, TASK-172; #7 -> TASK-170, TASK-171, TASK-172; #8 -> TASK-171, TASK-172; #9 -> TASK-169, TASK-170, TASK-171, TASK-172.

**No premature Done:** mandatory typed exactness cannot be deferred as residual; unverified cells block completion. Positive/NOT TRUE rows, nullable interval output domains, full DuckDB bags, adapter/13-dialect evidence, reviewed conditional vendor cases and final CI are necessary. TASK-91 remains a distinct release decision after TASK-86 closure.
