---
id: m-4
title: Verified SQL semantics and transformations
---

## Description

Explore mathematically checked foundations for SQL Semantic Protocol's canonical semantic rules, implementation correctness, and future semantics-preserving query transformations.

This is a **post-v3 research and roadmap milestone**, not a commitment to implement formal verification or to publish protocol 4.0.0. The milestone identifier `m-4` does not specify a SemVer release. Do not modify the 3.0.0 release gate, `m-3` acceptance criteria, or the existing dependency-ordered implementation queues. Any product implementation must be scoped and accepted in separate future tasks.

The current Rust analyzer produces typed claims about value domains, relational operations, physical-source realizability, and uncertainty. A formalization could prove general inference laws, but proofs about abstract SQL semantics alone would not verify the Rust implementation or establish behavior across all database engines. Preserve the distinction between modeled assumptions, formal theorems, generated proof certificates, executable implementation, and native engine evidence.

## Candidate workstreams

- **Formal semantic laws (Lean 4 / Mathlib):** Define a deliberately small SQL semantic subset and machine-check soundness of representative domain, NULL, and bag-multiplicity inference rules.
- **Verified Rust functions (Verus, with other tools considered as appropriate):** Assess whether concrete inference implementations can be shown to maintain their stated invariants.
- **Query equivalence and transformation:** Explore proofs that a rewrite preserves SQL result semantics under explicit source-schema, dialect, and session assumptions, including future compiler use cases.
- **Checkable proof evidence:** Evaluate certificates or other independently checked evidence for protocol claims, with an explicit trust boundary between the Rust analyzer and a verifier.

## Initial research

Start with [TASK-190](../tasks/task-190%20-%20evaluate-formal-verification-of-sql-semantics.md), a bounded research spike. Its findings should determine whether any of these candidates warrant implementation tasks.

Related but independently scoped post-v3 work: TASK-92 (typed evidence for opaque/runtime SQL semantics). Neither task is a prerequisite for the existing 3.0.0 release.
