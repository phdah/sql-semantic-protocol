---
id: m-3
title: Exact advanced SQL semantics for downstream generators
---

## Description

Extend SQL Semantic Protocol's authoritative, typed, exactness-aware contract so downstream test-data generators can construct reproducible sources for broader SQL outcomes. Focus on branch-aware set semantics, grouping and window witnesses, richer joins and subqueries, computed predicates, output-cardinality goals, and DML state transitions.

This milestone targets the one consolidated 3.0.0 release and the explicitly enumerated generator acceptance coverage. Every new capability must preserve default-deny residual behavior where exactness is not proved, include strongest provable outcome domains and differential tests, and maintain source-independent semantics and adapter parity. Downstream consumer: phdah/sql-tdg backlog m-3.

## Generator-consumable definition of done

Each new exactness capability must expose typed, source-independent obligations that a downstream consumer can use to construct **both qualifying and deliberately non-qualifying witnesses** when those classifications can be proven. Describe the relevant physical-source or selected intermediate relation identities, cross-row/column relationships, multiplicity or state constraints, and any dialect, NULL, schema, or comparison assumptions needed for soundness. A consumer must not need to reparse SQL or independently infer the operator's semantics to generate these witnesses.

When matching or rejected membership cannot be guaranteed, mark that classification residual with a traceable reason; do not imply that one proved direction automatically proves the other. Assert the exact contract, strongest provable outcome domains, feasible and impossible cases, and observed SQL results in tests. The protocol describes obligations and proofs; sampling, storage, and generation stay in sql-tdg.

## Release hold and scope (decision 2026-10-09)

**Do not merge Release Please PR #79 or publish protocol 3.0.0 until TASK-66..TASK-91 are Done and the sql-tdg m-3 integration gate is signed off.** TASK-58..65 are useful operator-local foundations, not evidence of complete end-to-end constructive physical-source planning. One final breaking 3.0 release is preferred to repeated intermediate releases.

A finite, reviewed, machine-readable, feature-by-dialect inventory (TASK-66) is the authoritative supported surface. The product can parse more SQL than it can prove. Arbitrary external UDFs, nondeterminism, unbounded recursion and vendor-specific semantics without available execution evidence cannot be promised sound exact generation; they must be explicitly residual or accepted as documented exclusions by the maintainer. Never silently drop a transformation. Where feasible, provide exact matching and rejected physical-source constructions, output row counts and typed final-state plans.

## Planned workstreams and dependencies

- Contract and proof foundations: TASK-66 inventory; TASK-67 typed constructive IR; TASK-68 physical-source DAG composition; TASK-69 bag and closed-world row-count laws.
- SQL transformation coverage: TASK-70 predicates/expressions, TASK-71 joins, TASK-72 sets, TASK-73 groups/HAVING, TASK-74 windows/QUALIFY, TASK-75 subqueries, TASK-76 ordering/limit/sampling, TASK-77 advanced table relations, TASK-78 CTE/recursive/producer scopes.
- Database state coverage: TASK-79 types/schema/constraints, TASK-80 INSERT, TASK-81 UPDATE/DELETE, TASK-82 MERGE/UPSERT, TASK-83 CREATE/REPLACE/ALTER/DROP/TRUNCATE, TASK-84 ordered multi-statement transactions.
- Whole-workload targets: TASK-85 output cardinality and distributions, TASK-86 multi-terminal matching/rejection, TASK-87 adapter parity, TASK-88 dialect conformance, TASK-89 differential and dbt integration, TASK-90 versioned extensible contract, TASK-91 signed release gate.

## Cross-repository completion path

The sql-tdg milestone m-3 remains open; its TASK-24..31, TASK-35 and TASK-36, plus new orchestrator/matrix tasks, must be exercised against a **pinned Git SHA of the protocol release candidate before publication**. This removes the circular dependency: the unpublished candidate can be tested against the generator, then a single v3.0.0 release is cut and sql-tdg changes its dependency to the published version. Do not merge release PR #79 early. The protocol must remain generic: sql-tdg consumes typed canonical contracts; no generator-specific sampling implementation belongs here.
