---
id: TASK-61
title: Define exact extended join and repeated-relation semantics
status: Done
assignee: []
created_date: '2026-10-08'
labels: []
milestone: m-3
dependencies: []
references:
  - 'TASK-43'
  - 'TASK-45'
  - 'TASK-52'
  - 'sql-tdg TASK-25'
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Canonical physical equality joins and their exactness are supported (TASK-45, TASK-52), but outer, semi, anti, non-equality and repeated/self-joins are residual. Provide a typed row-membership contract that distinguishes matched and unmatched row obligations.

SQL parsing, normalized semantics, lineage, and exactness remain owned by SQL Semantic Protocol. Preserve existing exact behavior while extending the canonical contract, and never mark unsupported cases exact. Changes must uphold the repository's outcome-first definition of done and cross-adapter parity.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Specify canonical exact-match, non-match, null-extension and instance identity obligations for LEFT/RIGHT/FULL, SEMI and ANTI joins where provable.
- [x] #2 Model simple non-equality join constraints and repeated/self-join relation instances without conflating constraints on different aliases.
- [x] #3 Preserve physical lineage and multi-layer composition, with explicit residuals for complex or unsupported shapes.
- [x] #4 Test NULL, missing parent, duplicate key, multi-match and relation alias cases against DuckDB, including dialect-compatible variants.
- [x] #5 Document protocol semantics and consumer requirements; sql-tdg TASK-25 depends on this task.
<!-- AC:END -->

## Implementation

- Added typed `join_witnesses` with explicit qualifying and rejected source-row cases, matching multiplicity, LEFT/RIGHT/FULL null extension, semi/anti membership, and safe inequality comparisons.
- Physical source endpoints retain distinct relation-instance aliases for self joins; join evidence composes through row-preserving plain-copy producer layers with layer provenance. Filtered/row-shaping upstream producers, composite join trees, unsupported predicates, unresolved lineage, and null-safe comparisons remain explicitly residual.
- Preserved the existing query-level `condition_exactness` residual status when whole-query membership is not provable, while exposing exact local witness directions; existing source/output-domain guarantees remain enforced.
- Added deterministic schema emission, full-protocol snapshot, DuckDB-backed NULL/missing partner/duplicate/multi-match/self-join tests and parser dialect coverage, plus protocol/semantic documentation. sql-tdg TASK-25 can consume the typed witnesses without parsing SQL.
- SQL, dbt and other supported adapters share the canonical analyzer and composed emission path. No additional adapter-specific interpretation is introduced.
