# TASK-138 acceptance audit

Task: `.backlog/tasks/task-138 - implement-star-transforms-and-fixed-schema-pivot-unpivot.md`  
Parent: TASK-77 (remains open)  
Finite fixture IDs: `77-C138-P`, `77-C138-N`  
Versioned inventory: `docs/task-138-acceptance-freeze.json`

## Canonical semantics and physical proof boundary

The shared positional wildcard modifier planner `src/analysis/star_projection.rs`
normalizes `EXCEPT`, `EXCLUDE`, `RENAME`, and `REPLACE`. It is consumed by
both `src/analysis/output.rs` (outcome domains/lineage) and
`src/analysis/finite_tables.rs` (physical rows) to avoid diverging
interpretations of AST wildcard modifiers.

A bounded, complete, source-free integer VALUES producer supports exact
physical star output transformations, static-key integer `SUM` PIVOT and
fixed-column UNPIVOT including explicit `INCLUDE NULLS` or `EXCLUDE NULLS`.
`src/protocol/finite_table.rs` owns typed source and terminal rows and
positional source contributors. SQL AST stays in analysis. The public
`physical_finite_table_plan` returns exact whole bags, typed terminal
domains, contributor row identities, and Impossible when a fully typed
requested terminal tuple is excluded by that complete bag.

Residual is required for unknown input rows, absent/ambiguous column targets,
mixed modifiers, dynamic pivots, unproved numeric width, opaque expressions,
and shapes not covered by this finite proof. Native vendor/session laws are
not certified by parser acceptance or DuckDB executions.

## Acceptance criteria trace

| AC | Required evidence | Current evidence |
| --- | --- | --- |
| #1 | Frozen complete finite matrix | Versioned `docs/task-138-acceptance-freeze.json` v6, 7 mandatory exact and 3 reviewed conditional fixture variants; fixed expected complete source/terminal bags, NULL and typed bounds, identities and explicit negative membership. |
| #2 | Canonical semantics and physical feasibility | Shared planner, source-free AST normalization, typed canonical `FiniteTable` with source references, and source/terminal Exact/Impossible/Residual in public physical API. |
| #3 | Independent SQL and terminal oracle | `tests/task_138_finite_star_bags.rs`, `tests/task_138_pivot_unpivot.rs`, `tests/task_138_frozen_matrix.rs` executes independent DuckDB source and terminal whole-bag oracles for every mandatory case, including NULL, duplicate, typed domains and both positive and impossible membership proofs. |
| #4 | Dialect and adapter evidence | 13-dialect parser matrix in `tests/task_138_frozen_matrix.rs`, SQL/dbt compiled adapter parity in `tests/task_138_adapters.rs`, and actual dbt Core model/canonical/warehouse E2E in `tests/task_138_dbt_core_e2e.rs`. ODCS cannot attest literal row contents, so source-free finite bag proof is inapplicable from ODCS alone. |
| #5 | Final-head CI and independent signoff | Final TASK-138 code-head [run 40126856488](https://github.com/phdah/sql-semantic-protocol/actions/runs/40126856488) on `31bc0e5204296b56c1f1ba7dd08484b6cc7939e9` passed 9/9 jobs (fmt, strict Clippy, full Rust tests, docs, no-default-features and real dbt E2E including schema/golden). Both standards and semantic/specification review axes passed after v6 source/terminal oracle and conditional boundary reconciliation. Final documentation-only PR head CI remains a merge condition. |

## Review disposition

The TASK-138 finite matrix is now reviewed: 7/7 mandatory cases require full Exact and independent Impossible proofs, with fixed source and terminal bags and exact types/domains. Three deliberate conditional cells (`star_except_missing`, `pivot_dynamic`, `pivot_opaque`) require Residual for invalid targets, unbounded dynamic schemas, or unknown physical source rows, respectively; each has TASK-138 ownership and an explicit release disposition. None represents an unimplemented mandatory finite case. The prior frozen negative fixture was reclassified in v5 after independent standards review to comply with the repository prohibition on mandatory Residual. Both fixture IDs remain covered and no exact requirement was removed. All 130 observed dialect classifications (54 exact, 46 parser boundaries, 30 residual) and 30 adapter dispositions are asserted by public tests. The temporary discovery probe was removed. Native DuckDB does not attest other dialect engines or session semantics.

**TASK-138 child acceptance: all five criteria signed off on code head.** The active v6 matrix has 7/7 mandatory Exact cases and 3/3 explicitly reviewed conditional residuals. Both original IDs, 130 dialect parser dispositions, 30 adapter cells and native DuckDB source/terminal whole bags were verified. The final documentation-only PR head CI remains a merge gate; TASK-77 parent and v3 release gates are independent.
An independently green source-backed DuckDB/finite proof does not imply native
vendor runtime certification.

The active finite-table JSON contract changes by adding `star`, `pivot`,
`unpivot`, `text`, `source`, and `contributors`. The breaking Conventional
Commit on the branch records the consumer compatibility obligation. TASK-77,
TASK-91 and v3 release gates remain independent.

## Independent final standards and semantic/specification review (2026-10-24)

**Standards.** All SQL AST interpretation is localized to analysis (`star_projection.rs` and `finite_tables.rs`). The parser-independent `FiniteTable` represents typed literal source/terminal bags and positional contributor references, and `physical_finite_table_plan` consumes only canonical facts. The active emission schema declares all new kinds, typed text, nested complete sources and contributor indices. Rust tests, docs, formatting, Clippy and schema/golden snapshots passed on final code head. Consumer-facing protocol changes are explicitly marked breaking and are not quietly attributed to an existing v3 release.

**Semantic/spec.** Seven frozen mandatory source-free fixed-schema SQL variants independently execute complete native DuckDB source *and* terminal whole-bag oracles; all produce exact typed domains and inclusive numeric extrema, preserve duplicates and NULL, prove positive tuple membership Exact and absent tuple Impossible, and retain source contributor identities. The three named conditionals have owner TASK-138, a justified Residual and explicit reviewed release disposition: invalid EXCEPT target, dynamic PIVOT output schema and opaque source rows. Nonrepresentable PIVOT/UNPIVOT column-list aliases, checked integer SUM overflow, mixed wildcard modifiers and unsupported collation remain fail-closed. `tests/task_138_adapter_matrix.rs` checks 30 adapter dispositions, including where ODCS schemas cannot provide source row contents; `tests/task_138_frozen_matrix.rs` checks 130 observed dialect parser/semantic outcomes. Unexecuted warehouse/session dialect behavior is not certified by the parser or DuckDB.

**Signoff boundary.** All five TASK-138 child criteria pass on code head 31bc0e5204296b56c1f1ba7dd08484b6cc7939e9; code CI 40126856488 passed 9/9. Documentation-only head CI must also pass before PR merge. TASK-77 parent remains open, and no TASK-91/v3 release signoff is inferred.
