---
id: TASK-66
title: Inventory executable SQL semantics for the generator release
status: Done
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: []
references:
  - 'sql-tdg PR #46'
  - 'sql-semantic-protocol PR #88'
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Define a finite, reviewed dialect-by-feature **coverage inventory** as the release's authoritative target. The inventory must enumerate variants, all dialect families, current evidence/unknowns, evidence needed for future certification, explicit conditional deferrals, and upstream/downstream owners. This task inventories and routes gaps; it does **not** implement or certify every SQL feature. Actual proof for every advertised supported variant is mandatory in its feature-owner task (TASK-67..90) and at the final release gate (TASK-91).

**Release contract:** This task supplies the coverage inventory used as the blocking checklist for protocol 3.0.0 and sql-tdg m-3. Feature tasks must implement canonical, source-independent typed outcomes; do not reparse SQL in the consumer. Every unverified cell must remain explicitly **release-blocking** for TASK-91 unless a reviewed, safely deferred and fail-closed scope is recorded. Marking this *inventory task* Done never certifies canonical semantics, output bounds, physical-source construction, negative row proofs, or vendor execution.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Inventory every documented sql-tdg m-3 and dbt fixture use case, raw SQL and scripted DDL/DML workloads, and all 13 exposed dialect families.
- [x] #2 Cross-classify joins, sets, CTEs, grouping, windows, predicates, row limits, nested relations, all write/DDL families, types, constraints, NULL, collation, time zones, and engine versions.
- [x] #3 For every inventoried feature/syntax variant and dialect, record current parse/canonical/physical-source positive-and-negative/cardinality/oracle evidence **or explicit unverified status**, dialect/session evidence requirements, reviewed conditional deferrals where applicable, and a responsible implementation task. Do not represent unspecified or inherited defaults as verified.
- [x] #4 Classify currently observed supported parser examples, unsupported/residual examples, and untested parser-boundary variants. Assign exhaustive dialect parse/semantic boundary classification to TASK-88, missing feature proofs to TASK-67..87, and integrated execution oracles to TASK-89/TASK-91. No unreviewed residual, unparseable or untested form counts as covered.
- [x] #5 Produce a machine-readable coverage manifest driving parameterized tests and a human-readable matrix with explicit release-blocking vs approved-out-of-scope classes.
- [x] #6 Record maintainer-approved cross-repository scope decisions and hand off sql-tdg TASK-24..36 feature owners, randomized per-terminal rejection (TASK-35), the unified DML/DDL E2E (TASK-31/36), and 13-dialect canonical conformance (TASK-44), with release acceptance remaining in protocol TASK-91.
- [x] #7 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Completion and non-waiver of release evidence (2026-10-09)

**TASK-66 is Done as an audited inventory/coordination deliverable, not as a semantic feature certification.** The original criteria mixed inventory ownership with evidence that depends on features not yet implemented. Criteria #3/#4/#6 now explicitly require **recording and triaging** those evidence gaps, while existing feature owner tasks and the final release gate retain the stronger obligations unchanged:

- **TASK-67..87** implement the actual canonical obligations, dialect parse semantics, complete input construction, positive/rejected classification, cardinality, DML/DDL and adapters.
- **TASK-88** requires exhaustive dialect-specific parser boundaries and exact canonical outcome equality for all claimed supported dialect/variant pairs.
- **TASK-89** requires full output/negative oracles and unified dbt + scripted DuckDB DML/DDL E2E.
- **TASK-90** owns the final v3 ABI/schema and forward-compatible extension behavior.
- **TASK-91** blocks publication until every required protocol cell is independently proven and sql-tdg TASK-24..31/35/36/43/44 pass end to end. Conditional deferred forms still require verified fail-closed handling; unknown cells are not excluded by marking this inventory Done.

The maintainer approved the scope decisions on 2026-10-09, and downstream owner acceptance tasks were synchronized in [sql-tdg PR #46](https://github.com/phdah/sql-tdg/pull/46). This is the completed handoff, **not** a declaration that exhaustive owner-level validation or the 3.0.0 release is approved.

## Implementation notes (2026-10-09)

- Inventory: [machine-readable manifest](../../docs/coverage-manifest.json) and [readable matrix](../../docs/coverage.md): **53 feature families**, **251 explicitly enumerated variants**, **13 canonical dialect families**, 689 family cells and 3,263 logical variant/dialect cells with default-deny evidence. `postgres` is covered as the alias of `postgresql`.
- Existing evidence is marked as **representative parser/analysis only**, never assumed to prove exact generator input construction. Each variant defaults to unverified; opaque UDF/recursion/sampling forms are **approved conditional deferrals**, not permanent exclusions, with safely bounded variants remaining release-blocking.
- [Integration tests](../../tests/coverage_manifest.rs) consume the manifest, exercise shared cross-dialect fixtures, Snowflake MINUS, a parseable LIMIT residual and DuckDB feasible/impossible/NULL/duplicate SQL oracles. Coverage claims do not imply downstream generator success.
- Downstream mapping: sql-tdg TASK-24..31, 35 and 36; TASK-33 is the existing conformance matrix; TASK-43 pins the release candidate; TASK-91 owns final cross-repo sign-off. Existing upstream TASK-67..90 own the semantic feature families.
- **Still required before 3.0.0 release (not before this inventory's completion):** independent parser + canonical + constructive positive/negative + cardinality + dialect/engine law evidence for every claimed supported variant, owned by TASK-67..91 and downstream sql-tdg tasks. TASK-66 documents the release-blocking holes instead of falsely marking any as supported. The current fixture tests sample evidence, not full feature certification.
- This is an **infrastructure-only inventory**. There are no changes to the active SQL protocol schema, library public types, or dbt/ODCS adapters, because no new canonical semantic capability is claimed by this task. Later TASK-67..90 must update those interfaces and tests together.

Implementation PR: https://github.com/phdah/sql-semantic-protocol/pull/88

### Sign-off handoff and verification

- [Maintainer decision record](../../docs/coverage-signoff.md) records the **maintainer-approved** four scope decisions (2026-10-09): seeded randomized per-terminal rejecting alternatives, unified dbt plus required DML/DDL scripted E2E, extensible typed evidence for future opaque semantics, and exact canonical protocol equivalence across every supported parsing dialect. Approval is not generator or release sign-off.
- Additional parser fixtures cover SQL sets, joined relations, grouped/HAVING and window projections, correlated EXISTS, CTEs, conditional logic and known residual LIMIT. DuckDB oracle fixtures include complete row values for representative joins, sets, grouping, EXISTS and QUALIFY, in addition to positive/negative/NULL/duplicate counts. No SQL generator exactness is asserted from these oracles.
- The readable matrix and task owners are checked against the manifest by executable Rust tests. The existing protocol JSON schema, public Rust API and adapters remain unchanged because TASK-66 adds no new domain semantics. Criteria #3/#4/#6 are complete as **inventory status, gap triage and cross-repo ownership/decision handoff**. Exhaustive parser/constructive/engine evidence is separately mandatory for TASK-88/89/91 and downstream feature tasks; maintainer scope approval has not certified any unverified feature/dialect cell. No unsupported variant is counted as covered.

## Maintainer scope approval (2026-10-09)

The four decisions in [docs/coverage-signoff.md](../../docs/coverage-signoff.md) are approved with addendums. This fixes the product contract but not the unverified executable evidence. TASK-66's inventory/handoff criteria are complete; the **TASK-88/89/91 and sql-tdg TASK-36/44** release criteria remain unchecked until each claimed supported variant/dialect has canonical and physical-source proof or a reviewed conditional deferral and exhaustive downstream E2E tests. A fixed random seed must reproduce rejecting-alternative selection, while multiple seeds prove every constructive predicate/column alternative can be selected. All thirteen dialects must show equality of canonical outputs for semantically equivalent SQL; DuckDB remains the actual executable full-pipeline oracle.
