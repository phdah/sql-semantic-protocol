# TASK-141 acceptance review

Scope: [TASK-141](../.backlog/tasks/task-141%20-%20prove-nested-and-shadowed-cte-source-identity.md), child 1/4 of TASK-78. The immutable proposed case inventory is [v1](task-141-acceptance-freeze.json). **Status: In Progress.** Passing the SQL oracles does not certify source-constructive protocol proof.

## Standards review

The implementation maintains declaration-ordered ordinary CTE bindings, reserves recursive bindings without using inherited physical or column-domain evidence, and tests public Rust analysis and independent DuckDB SQL results. No new dependencies, parser dialect whitelist, lint exclusions, or public JSON contract changes were introduced. Do not assume a scoped CTE has an executable physical producer merely because dependencies and output lineage were resolved.

## Spec review and missing gates

| AC | Evidence available on this PR | Remaining mandatory evidence |
| --- | --- | --- |
| #1 | Frozen v1 source/producer/terminal bags plus v2 56-cell dialect, 12-cell adapter, and conditional release matrix | Independent review of every mandatory and conditional cell |
| #2 | Typed `CteProgram` and `physical_finite_cte_bag_plan` now implement source-complete lexical DAG, row identity, TRUE/FALSE/UNKNOWN, Exact/Impossible/Residual and typed domains | Final-head proof tests and independent review of all unsupported boundaries |
| #3 | Independent DuckDB source/terminal oracles and public `tests/task_141_physical.rs` assert all four producer bags, identifiers, complete terminal values and counterexamples | Final-head Rust test verification |
| #4 | 14 dialect-name proof equivalence, direct SQL/dbt catalog/ODCS source complete parity and 42 explicit conditional parser/residual classifications added | Confirm all matrix paths green on final CI head and review native-engine limitations |
| #5 | CI suite runs fmt, strict Clippy, full/default/no-default tests, docs and real dbt E2E | All checks green on final SHA, independent review of each mandatory cell, original parent-AC signoff and approved conditional dispositions |

**No mandatory case is signed off yet.** The bounded implementation and tests are committed but final-head CI and independent review are pending. The task stays In Progress, PR stays draft and TASK-78 must remain open.

## Follow-up author review (not independent signoff)

A standards/spec pass identified two situations that could falsely certify a
complete CTE bag when SQL execution has unproved source or binding semantics:

- An `ORDER BY` identifier that does not resolve to a projected output label
  could fail in the engine. The bounded CTE lowering now permits only provable
  projected labels; all exposed parser dialect names are covered by a negative
  public-API test.
- The table-source AST may carry partition selection, time travel, ordinality,
  hints, indexes or a JSON path. These are not interchangeable with one
  complete unmodified source bag. The lowering now requires an ordinary table
  factor, with a MySQL partition-selection regression test.

These are soundness fixes within the existing mandatory Exact claims, not new
mandatory acceptance variants. They do not replace a separate independent
review of frozen cases, conditional dispositions and the original parent ACs.
