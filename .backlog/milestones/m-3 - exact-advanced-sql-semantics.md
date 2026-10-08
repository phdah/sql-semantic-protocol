---
id: m-3
title: Exact advanced SQL semantics for downstream generators
---

## Description

Extend SQL Semantic Protocol's authoritative, typed, exactness-aware contract so downstream test-data generators can construct reproducible sources for broader SQL outcomes. Focus on branch-aware set semantics, grouping and window witnesses, richer joins and subqueries, computed predicates, output-cardinality goals, and DML state transitions.

This milestone does not promise universal SQL support or a release version. Every new capability must preserve default-deny residual behavior where exactness is not proved, include strongest provable outcome domains and differential tests, and maintain source-independent semantics and adapter parity. Downstream consumer: phdah/sql-tdg backlog m-3.

## Generator-consumable definition of done

Each new exactness capability must expose typed, source-independent obligations that a downstream consumer can use to construct **both qualifying and deliberately non-qualifying witnesses** when those classifications can be proven. Describe the relevant physical-source or selected intermediate relation identities, cross-row/column relationships, multiplicity or state constraints, and any dialect, NULL, schema, or comparison assumptions needed for soundness. A consumer must not need to reparse SQL or independently infer the operator's semantics to generate these witnesses.

When matching or rejected membership cannot be guaranteed, mark that classification residual with a traceable reason; do not imply that one proved direction automatically proves the other. Assert the exact contract, strongest provable outcome domains, feasible and impossible cases, and observed SQL results in tests. The protocol describes obligations and proofs; sampling, storage, and generation stay in sql-tdg.

Capabilities may be implemented and released independently once their own proof obligations and tests pass. The corresponding sql-tdg task can then consume that released protocol version without waiting for every m-3 task.

## Downstream status and mapping

The post-1.0 sql-tdg milestone m-3 already tracks the consumers: sql-tdg TASK-24 -> protocol TASK-58; TASK-25 -> TASK-61; TASK-26 -> TASK-62; TASK-27 -> TASK-63; TASK-28 -> TASK-59; TASK-29 -> TASK-60; TASK-30 -> TASK-64; and TASK-31 -> TASK-65. sql-tdg TASK-32 (conflicting-outcome scenarios) is independent of these new protocol contracts, while TASK-33 (dialect conformance matrix) is already Done. The generator's dialect matrix and E2E fixtures must be updated as each newly supported semantic feature lands.
