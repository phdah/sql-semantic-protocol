---
id: TASK-69
title: Prove bag, duplicate and closed-world row-count semantics
status: Done
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-67
references: 
  - 'TASK-58'
  - 'TASK-61'
  - 'TASK-64'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Set duplicate arithmetic and local match counts do not suffice for joins/groups/windows with global row conservation or negative witnesses.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Model multiplicity of row/tuple identities under projection, DISTINCT, joins, set operators, aggregation, rank filtering, insert/delete/update and no-match conditions.
- [x] #2 Define exact cardinality transfer functions and admissible bounds under NULL-aware equality, bag semantics, duplicate keys, zero rows, and cross-row correlations.
- [x] #3 Expose complete-physical-relation closed-world obligations for absence and anti-join/subquery/EXCEPT cases; distinguish empty relation from absent candidate.
- [x] #4 Cover feasible/impossible count goals and preserve constraints on copies, self joins and aliases without inventing independent physical tables.
- [x] #5 Differential-test all counts and output histograms on DuckDB including many-to-many joins, duplicate cancellations, empty sets and NULL tuples.
- [x] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
<!-- AC:END -->

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## Implementation and verification checkpoint (2026-10-10)

Implementation: [PR #90](https://github.com/phdah/sql-semantic-protocol/pull/90).

- **#1:** Canonical source/tuple identities, row-preserving projection, DISTINCT, group-key and global-aggregate cardinality, strict rank-prefix, all six SQL set multiplicity laws, typed equijoin/outer/semi/anti laws, complete append/delete/update subset counts and negative no-match cases. Unproved operator variants remain explicitly residual.
- **#2:** Closed-world inclusive count bounds, checked overflow, NULL-aware set equality versus SQL NULL nonmatching joins, zero rows, per-key mixed duplicate join histograms, complete physical alias/copy coupling, impossible contradictions and caller-goal assessments distinguish entailed/impossible/residual. Cross-type coercion/collation fails closed.
- **#3:** The canonical constructive witness IR now emits the versioned `closed_world` obligation at a **physical** boundary, with `entire_relation` for unmatched partner / EXISTS / IN cases and `candidate_tuple` for zero-count set candidates. A missing candidate does not imply an empty source; unresolved producers cannot be inserted into as physical tables.
- **#4:** `BagSourceIdentity` preserves physical relation versus SQL alias, and overlapping counts for the same physical relation are intersected; contradictions are impossible. Repeated set branches use correlated identical counts, never independently sampled multiplicity.
- **#5:** DuckDB integration oracles compare complete candidate histograms across all six set laws, NULL/duplicates, many-to-many joins and outer-null extensions, empty/absent candidates, group/rank counts and sequential DELETE, UPDATE, INSERT cardinality. The 13 dialect families exercise common UNION ALL canonical rules.
- **#6:** New and expanded Rust unit/integration tests; public Rust API; schema's closed-world coverage variants; canonical SQL and dbt emission snapshots, docs/protocol.md and coverage inventory checkpoint. The ODCS/dbt/raw SQL paths share the same resolved semantics and emission, avoiding adapter-specific bag logic.

**Boundary of TASK-69:** These are operator-local exact laws and source-completeness requirements, not a claim of universal whole-graph data construction. TASK-68 owns transitive physical-source realization and joint satisfiability, TASK-70..87 own individual unsupported semantic variants, TASK-88/89 own exhaustive dialect and cross-feature engine oracles, and TASK-91 owns final generator sign-off. Unverified release cells remain blocked in `docs/coverage-manifest.json`; PR #79 must not be merged based on this task alone.

**Acceptance sign-off:** All six task-specific operator-local criteria are complete in PR #90. The fully passing GitHub Actions run `37998322646` (head `e9af8c4127e1dc85a5945d307937e0a35ddf2a82`) verified format, warnings-as-errors lint, Rust tests, API documentation, no-default-features dependency/lint/test/doc and dbt Core end-to-end snapshots. Final task metadata is documentation only. This does **not** grant release approval for any unchecked feature/dialect inventory cell or the downstream multi-layer constructive generator.
