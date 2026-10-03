---
id: TASK-28
title: Derive output-column value domains
status: Done
assignee: []
created_date: '2026-10-02'
labels: []
milestone: m-0
dependencies:
  - TASK-7
  - TASK-17
  - TASK-18
---

## Description

Derive conservative value domains for produced output columns, independently from the existing source-column domains inferred from filtering predicates.

The protocol is outcome-driven, so a derived output should describe not only its expression and physical lineage but also the values that expression can produce when they are knowable safely. This includes conditional expressions such as CASE and intrinsic result constraints from supported functions such as ROW_NUMBER.

Output-domain reasoning must remain separate from source-column predicate reasoning. A predicate on a derived alias may refine the derived output domain without incorrectly constraining the physical columns that contribute to that expression.

## Acceptance Criteria

- [x] Output columns can represent a parser-independent value domain separately from source-column domains.
- [x] Directly projected columns inherit safe predicate-derived constraints into their output domains, so filtered outputs expose their actual lower/upper interval bounds rather than only source-column metadata.
- [x] CASE expressions are represented explicitly rather than as unsupported expressions, including searched and simple CASE forms where sqlparser exposes sufficient semantics.
- [x] CASE conditions and result branches contribute complete output-column lineage.
- [x] CASE result domains are combined conservatively from reachable result branches; for example, a CASE whose results are only TRUE and FALSE has the output domain {true, false}.
- [x] Boolean-producing derived expressions that can be modeled safely expose a boolean output domain rather than losing their semantics.
- [x] Supported window functions expose intrinsic output domains where SQL semantics guarantee one; at minimum ROW_NUMBER has an integer lower bound of 1.
- [x] Predicates on derived aliases refine the derived output domain without being misapplied to physical source-column domains; for example, QUALIFY rn <= 10 combined with ROW_NUMBER yields rn in [1, 10].
- [x] Supported aggregate functions expose intrinsic output domains only where guaranteed by SQL semantics, building on TASK-18; unknown aggregate result bounds remain unknown.
- [x] Safe scalar domain propagation is supported for common derived expressions such as literal-preserving unary or arithmetic expressions where bounds can be computed without guessing.
- [x] Output domains survive multi-layer composition so a final outcome retains safely derivable constraints from intermediate derived columns.
- [x] Unsupported, non-deterministic, overflow-sensitive, dialect-specific, or otherwise unsafe domain transformations remain explicitly unknown/unsupported rather than over-claimed.
- [x] JSON Schema, protocol documentation, and public API documentation are updated for output-column domains.
- [x] Tests cover CASE-to-boolean output, direct boolean derived expressions, ROW_NUMBER, QUALIFY refinement, aggregate intrinsic domains, arithmetic propagation, unknown fallback, lineage, and deterministic JSON emission across relevant dialects.
