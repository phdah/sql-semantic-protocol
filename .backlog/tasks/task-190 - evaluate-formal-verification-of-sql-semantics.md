---
id: TASK-190
title: Evaluate formal verification of SQL semantic inference rules
status: To Do
assignee: []
created_date: '2026-10-27'
updated_date: '2026-10-27'
labels:
  - future
  - research
  - formal-verification
milestone: m-4
dependencies: []
references:
  - 'TASK-92'
  - 'docs/semantics.md'
  - 'docs/protocol.md'
  - 'docs/conformance-suite.md'
priority: medium
type: spike
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Research whether formally verified semantic laws and/or verified Rust implementations would materially strengthen SQL Semantic Protocol's trustworthy outcome claims. The focus is general soundness of source-independent inference rules, not merely proving one query against a particular input bag.

Compare Lean 4 + Mathlib for machine-checked mathematical models and Verus for Rust-specific verification. Consider implications for future query equivalence and semantics-preserving SQL rewrites, while preserving the canonical protocol's separation of parsing, semantic analysis, emission, and consumers.

**Scope boundary:** This is a post-v3 research spike, not authorization to introduce a Lean/Verus dependency, change public Rust/JSON contracts, certify native dialect behavior, or delay the 3.0.0 release. Formal properties of a mathematical model do not automatically certify the Rust analyzer, the SQL parser, or vendor/session execution semantics.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Inventory at least three existing inference-law candidates (including a value-domain rule, a bag-multiplicity rule, and a NULL/three-valued or outer-join rule). Link each to its current Rust implementation, public semantic contract, and proof assumptions; prioritize one small tractable subset.
- [ ] #2 Formalize and machine-check at least one representative soundness theorem in a minimal Lean 4 prototype. State the exact abstract SQL semantics, input/type/NULL assumptions, theorem, proof, and excluded cases; record reproducible commands and versions.
- [ ] #3 Evaluate Verus against the Rust implementation of one representative inference rule. Provide a minimal proof-of-concept where feasible, or a concrete reproducible limitation, and distinguish verification of a Rust function from verification of its SQL model.
- [ ] #4 Compare architecture options (standalone formal specification, verified Rust implementation, and analyzer-generated proof certificates) with explicit trusted computing bases, maintenance costs, CI implications, coverage limitations, and dialect/vendor assumption boundaries.
- [ ] #5 Produce a concise research report with artifacts or links, a recommendation and decision criteria for further investment, plus separately scoped follow-up candidate tasks. Do not promote `Exact`/`Feasible` evidence on the strength of a prototype or change current release gates.
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Research references to investigate:
- Lean 4: https://lean-lang.org/
- Mathlib: https://github.com/leanprover-community/mathlib4
- Verus: https://verus-lang.github.io/verus/guide/
- Kani (bounded model checking alternative): https://model-checking.github.io/kani/
- Cosette (SQL equivalence research): https://cosette.cs.washington.edu/

Potential starter laws: a typed `WHERE x >= n` surviving-value lower bound under SQL three-valued logic; `UNION ALL` tuple-count addition for compatible bags; LEFT JOIN NULL-extension and unmatched-row preservation. Select the formalization for a clearly defined portable fragment, not a claim about all parser dialects.

Use the repository's existing semantic contract and conformance fixtures as references. Keep research artifacts isolated from runtime code and dependencies until an implementation proposal is reviewed.
<!-- SECTION:NOTES:END -->
