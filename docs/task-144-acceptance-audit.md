# TASK-144 independent bounded acceptance audit

**Date:** 2026-10-31. **Child:** TASK-144 (Done on PR branch).
**Parent:** TASK-78 (not signed off, not release-approved).
**PR:** https://github.com/phdah/sql-semantic-protocol/pull/170

## Scope and freeze history

The initial finite preimplementation matrix was committed before semantic edits and is preserved at
`docs/task-144-acceptance-freeze-v1.json`. The reviewed, explicit version 2
`docs/task-144-acceptance-freeze.json` repairs a mandatory missing nested/shadowed
producer oracle without weakening the original scope or retroactively rewriting v1.
All original TASK-144 AC #1..#5 and original TASK-78 AC #1..#6 remain tracked.
The active `docs/coverage-manifest.json` links the reviewed nine-case matrix.

Seven mandatory Exact cases:

| Fixture | Physical outcome | Independent oracle |
| --- | --- | --- |
| 78-C144-P-REUSED-FILTER | One filtered/computed stage, two consumers | `tests/task_144_frozen_reads.rs` |
| 78-C144-P-FINITE-RECURSION | Guarded recursive UNION ALL, exact [1,2,3] | `tests/task_144_frozen_reads.rs` |
| 78-C144-N-NULL-DUP | NULL UNKNOWN + duplicate 2x2 bag, negative membership | `tests/task_144_frozen_reads.rs` |
| 78-C144-P-MULTISTEP | INSERT from original source, final CTE reads constructed target | `tests/task_144_ordered_cte.rs` |
| 78-C144-N-MULTISTEP-NULL | Filtered NULL, duplicate target and four final matches | `tests/task_144_ordered_cte.rs` |
| 78-C144-P-VIEW | Logical view from original source, no independent view rows | `tests/task_144_view.rs` |
| 78-C144-P-SHADOW | Nested lexical shadowing, unused outer leaf retained and not copied | `tests/task_144_shadow_oracles.rs` |

Each case asserts complete terminal values and row identities, source/producer
provenance, multiplicity, exact typed NULL/bound inclusivity, and TRUE/NOT TRUE
`Exact`/`Impossible` classifications. The native DuckDB oracle observes
source/producer/terminal bags without relying on the protocol evaluator for
its expected results.

Two conditional cases have explicitly reviewed typed `Residual` rather than false
portable Exact: `78-C144-N-RECURSIVE-CYCLE` (no finite termination certificate)
and `78-C144-N-LATERAL-SESSION` (unproved correlated LATERAL binding and
temporary/session scope). These are owned by TASK-144 and explicitly **not**
native vendor execution certifications.

## Canonical model and adapter verification

- `src/physical_realization/cte_scripts.rs` composes an ordered write with a
  terminal CTE read over the *constructed* post-write target, not a caller-inserted
  intermediate. Its complete before, intermediate, after and terminal bags
  are checked against DuckDB and against dbt's compiled read-model semantics.
- `src/physical_realization/view_bags.rs` proves only one bounded ordinary logical
  view from an attested leaf. Source occurrences survive WHERE and projection.
  Materialized, temporary, conditional, renamed and vendor-specific view clauses
  keep their existing canonical producer layers while setting the optional
  `view_execution_unknown` marker; unsupported execution cannot become Exact.
  `tests/task_144_view.rs` checks native results and emitted uncertainty.
- `tests/task_144_dialect_exactness.rs` asserts identical complete canonical
  rows/source identities for seven mandatory forms across all **14 exposed
  dialect identities** (13 baseline identities plus the PostgreSQL alias).
  The two conditional forms stay Residual. `tests/task_144_dialects.rs`
  checks all **126 explicitly frozen parser cells** without silently
  excluding parser-accepted variants. Parser success never asserts native
  execution outside DuckDB.
- `tests/task_144_adapter_parity.rs` checks direct SQL, compiled dbt manifest
  and authoritative catalog, and ODCS 3.2 source schemas whenever equivalent
  facts are representable: ordinary/reused CTEs, shadowing, finite recursion,
  logical view, and the compiled reader of a source-derived write target.
  An arbitrary raw `INSERT` is not a dbt model artifact, and source-free
  recursion has no equivalent ODCS source-schema fact; these are named
  boundaries, not fabricated adapter proofs. **27 adapter cells** have
  explicit owner and disposition in the freeze.
- `tests/task_144_acceptance_inventory.rs` enforces all nine cases, seven
  Exact/two Residual classifications, original parent AC mapping, 126 dialect
  identities, 27 adapter dispositions, and the still-open parent/release gate.

## Five child acceptance criteria

1. **AC #1, freeze and bounded inventory: verified.** v1 was committed
   before semantic work. The missing shadowed-source cell was explicitly reviewed,
   added in v2, and v1 retained as immutable evidence. Every current cell is
   accounted for in the coverage manifest and the executable inventory.
2. **AC #2, canonical source-complete semantics: verified.** Shared stages,
   bounded recursion, materialization into a target, logical view, shadowing,
   typed output domains, duplicate/NULL laws, and negative membership match
   the frozen complete-bag oracles. Unsupported conditional laws fail closed.
3. **AC #3, independent native physical evidence: verified.** Full initial,
   producer, target and terminal DuckDB values and row occurrence identities
   plus a NULL/duplicate counterexample and source-absence proof are asserted
   by the public API tests.
4. **AC #4, dialect and adapter parity: verified.** All 126 parser/canonical
   cells and 27 named adapter dispositions are covered; equivalent facts are
   checked as the same typed canonical protocol, and remaining vendor/session
   laws have explicit Residual without a native vendor claim.
5. **AC #5, final implementation CI and independent Standards/Spec review:
   verified on implementation head.** The two-axis Matt code review is at
   https://github.com/phdah/sql-semantic-protocol/pull/170#pullrequestreview-5723914446.
   No blocking Standards or bounded TASK-144 Spec finding remains. This
   `COMMENT` is authored through the owning GitHub connection, not a
   distinct-account GitHub approval.

**Verified implementation head:** `0a05bc16d598c99bd5961f6536c6ce78b9a53e27`.
**CI:** https://github.com/phdah/sql-semantic-protocol/actions/runs/41222021788,
**9/9 passed**: Rust fmt, strict Clippy, full tests, doc, no-default-feature
lint/test/doc, dependencies and dbt Core end-to-end. The active schema and
golden assertions are exercised through the full Rust test suite.

**Final PR-head gate:** the signoff metadata commit must independently pass
CI before merge. Its green run can be recorded on the PR without
amending this already reviewed implementation-head evidence.

## Parent TASK-78 closure remains separate

This child is Done **on the PR branch**, but TASK-78 remains To Do and
`parent_signed_off=false`, `release_approved=false`. The parent requires
TASK-141, TASK-142, TASK-143 and TASK-144 to be Done **and merged on main**,
then independent review of its original AC #1..#6 and conditional
dialect/vendor boundaries. A child audit and green CI do not bypass
the merge/integration gate or certify unavailable native dialect engines.
