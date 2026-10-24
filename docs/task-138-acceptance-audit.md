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
| #1 | Frozen complete finite matrix | Versioned `docs/task-138-acceptance-freeze.json`, 8 mandatory and 2 conditional fixture variants; fixed expected complete source/terminal bags, NULL and typed bounds, identities and explicit negative membership. |
| #2 | Canonical semantics and physical feasibility | Shared planner, source-free AST normalization, typed canonical `FiniteTable` with source references, and source/terminal Exact/Impossible/Residual in public physical API. |
| #3 | Independent SQL and terminal oracle | `tests/task_138_finite_star_bags.rs`, `tests/task_138_pivot_unpivot.rs`, `tests/task_138_frozen_matrix.rs` and whole DuckDB bag comparisons. |
| #4 | Dialect and adapter evidence | 13-dialect parser matrix in `tests/task_138_frozen_matrix.rs`, SQL/dbt compiled adapter parity in `tests/task_138_adapters.rs`, and actual dbt Core model/canonical/warehouse E2E in `tests/task_138_dbt_core_e2e.rs`. ODCS cannot attest literal row contents, so source-free finite bag proof is inapplicable from ODCS alone. |
| #5 | Final-head CI and independent signoff | Pending full final-head CI, independent code review and reconciled acceptance inventory. |

## Review disposition

The task remains **In Progress**, with no AC prematurely checked.
The temporary parser-dialect discovery probe must be removed, all 130 dialect
cells and 30 adapter cells individually reconciled, active schema/golden and
real dbt E2E passed, and both standards/spec reviews resolved before setting Done.
An independently green source-backed DuckDB/finite proof does not imply native
vendor runtime certification.

The active finite-table JSON contract changes by adding `star`, `pivot`,
`unpivot`, `text`, `source`, and `contributors`. The breaking Conventional
Commit on the branch records the consumer compatibility obligation. TASK-77,
TASK-91 and v3 release gates remain independent.
