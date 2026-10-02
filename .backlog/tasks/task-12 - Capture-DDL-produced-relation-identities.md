---
id: TASK-12
title: Capture DDL-produced relation identities
status: Done
assignee: []
created_date: '2026-10-01'
labels: []
milestone: m-0
dependencies:
  - TASK-11
---

## Description

Teach the analyzer to record the relation produced by query-backed DDL so independently parsed inputs can be linked together.

The initial target is query-backed table and view creation, including dialect variants represented by sqlparser such as CREATE OR REPLACE, temporary objects, materialized views, and qualified identifiers when those forms are supported by the selected dialect.

A bare SELECT still produces an anonymous result. DDL that defines an object without a query must not be treated as if it produced transformation semantics.

## Acceptance Criteria

- [x] CREATE TABLE ... AS SELECT records the created relation as the statement's produced dataset.
- [x] CREATE VIEW ... AS SELECT records the created relation as the statement's produced dataset.
- [x] Supported CREATE OR REPLACE, temporary, materialized, and qualified-name variants preserve the correct produced relation identity.
- [x] Quoted and qualified identifiers are represented deterministically and without unsafe case-folding assumptions.
- [x] The query inside supported DDL is analyzed with the same semantic behavior as a standalone query.
- [x] Bare SELECT statements remain representable as anonymous outputs.
- [x] Non-query DDL and unsupported DDL shapes are reported explicitly instead of being linked incorrectly.
- [x] Shared query-backed DDL semantics are verified across every dialect name exposed by the project, with additional dialect-specific coverage for materially different DDL variants.
