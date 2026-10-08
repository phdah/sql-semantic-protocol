---
id: m-3
title: Exact advanced SQL semantics for downstream generators
---

## Description

Extend SQL Semantic Protocol's authoritative, typed, exactness-aware contract so downstream test-data generators can construct reproducible sources for broader SQL outcomes. Focus on branch-aware set semantics, grouping and window witnesses, richer joins and subqueries, computed predicates, output-cardinality goals, and DML state transitions.

This milestone does not promise universal SQL support or a release version. Every new capability must preserve default-deny residual behavior where exactness is not proved, include strongest provable outcome domains and differential tests, and maintain source-independent semantics and adapter parity. Downstream consumer: phdah/sql-tdg backlog m-3.
