---
id: TASK-66
title: Inventory executable SQL semantics for the generator release
status: In Progress
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: []
references: []
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Define a finite, reviewed dialect-by-feature inventory as the release's authoritative coverage target. 'Any SQL' is not a sound promise for arbitrary UDFs, recursive queries and engine-specific behavior; every enumerated generator use case must instead be fully proved or explicitly excluded with maintainer sign-off.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Inventory every documented sql-tdg m-3 and dbt fixture use case, raw SQL and scripted DDL/DML workloads, and all 13 exposed dialect families.
- [x] #2 Cross-classify joins, sets, CTEs, grouping, windows, predicates, row limits, nested relations, all write/DDL families, types, constraints, NULL, collation, time zones, and engine versions.
- [ ] #3 For every feature/syntax variant specify AST parse support, normalized canonical meaning, physical-source constructive positive/negative witness, output cardinality, dialect assumptions, executable oracle and documented exclusion.
- [ ] #4 Enumerate unsupported-but-parseable variants and unparseable dialect forms; no unreviewed residual may be counted as covered. Add a triaged upstream task for each release-blocking gap.
- [x] #5 Produce a machine-readable coverage manifest driving parameterized tests and a human-readable matrix with explicit release-blocking vs approved-out-of-scope classes.
- [ ] #6 Review inventory with sql-tdg TASK-24..36 owners before finalizing release scope.
- [x] #7 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Implementation notes (2026-10-09)

- Inventory: [machine-readable manifest](../../docs/coverage-manifest.json) and [readable matrix](../../docs/coverage.md): **53 feature families**, **251 explicitly enumerated variants**, **13 canonical dialect families**, 689 family cells and 3,263 logical variant/dialect cells with default-deny evidence. `postgres` is covered as the alias of `postgresql`.
- Existing evidence is marked as **representative parser/analysis only**, never assumed to prove exact generator input construction. Each variant defaults to unverified; opaque UDF/recursion/sampling forms are **approved conditional deferrals**, not permanent exclusions, with safely bounded variants remaining release-blocking.
- [Integration tests](../../tests/coverage_manifest.rs) consume the manifest, exercise shared cross-dialect fixtures, Snowflake MINUS, a parseable LIMIT residual and DuckDB feasible/impossible/NULL/duplicate SQL oracles. Coverage claims do not imply downstream generator success.
- Downstream mapping: sql-tdg TASK-24..31, 35 and 36; TASK-33 is the existing conformance matrix; TASK-43 pins the release candidate; TASK-91 owns final cross-repo sign-off. Existing upstream TASK-67..90 own the semantic feature families.
- **Still required** before Done: independent parser + canonical + constructive positive/negative + cardinality + engine/version oracles for **each** reviewed variant/dialect scope; triage executable parser-boundary and residual evidence with TASK-88 and the feature owners, then confirm the now-approved randomized-rejection and unified E2E scope with sql-tdg TASK-35/36 implementations. The current fixture tests sample evidence, not full feature certification.
- This is an **infrastructure-only inventory**. There are no changes to the active SQL protocol schema, library public types, or dbt/ODCS adapters, because no new canonical semantic capability is claimed by this task. Later TASK-67..90 must update those interfaces and tests together.

Implementation PR: https://github.com/phdah/sql-semantic-protocol/pull/88

### Sign-off handoff and verification

- [Maintainer decision record](../../docs/coverage-signoff.md) records the **maintainer-approved** four scope decisions (2026-10-09): seeded randomized per-terminal rejecting alternatives, unified dbt plus required DML/DDL scripted E2E, extensible typed evidence for future opaque semantics, and exact canonical protocol equivalence across every supported parsing dialect. Approval is not generator or release sign-off.
- Additional parser fixtures cover SQL sets, joined relations, grouped/HAVING and window projections, correlated EXISTS, CTEs, conditional logic and known residual LIMIT. DuckDB oracle fixtures include complete row values for representative joins, sets, grouping, EXISTS and QUALIFY, in addition to positive/negative/NULL/duplicate counts. No SQL generator exactness is asserted from these oracles.
- The readable matrix and task owners are checked against the manifest by executable Rust tests. The existing protocol JSON schema, public Rust API and adapters remain unchanged because TASK-66 adds no new domain semantics. Criteria #3 and #4 still require TASK-67..90's exhaustive evidence, and #6 still requires review of the exhaustive variant inventory with downstream task owners; the maintainer has approved the governing *scope decisions*, not certified all feature/dialect cells. No unsupported variant is counted as covered.

## Maintainer scope approval (2026-10-09)

The four decisions in [docs/coverage-signoff.md](../../docs/coverage-signoff.md) are approved with addendums. This fixes the product contract but not the unverified executable evidence. TASK-66 #3/#4/#6 remain unchecked until each supported variant/dialect has canonical and physical-source proof or a reviewed conditional deferral, with all downstream owner reviews completed. A fixed random seed must reproduce rejecting-alternative selection, while multiple seeds prove every constructive predicate/column alternative can be selected. All thirteen dialects must show equality of canonical outputs for semantically equivalent SQL; DuckDB remains the actual executable full-pipeline oracle.
