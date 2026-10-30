# TASK-141 acceptance review

Scope: [TASK-141](../.backlog/tasks/task-141%20-%20prove-nested-and-shadowed-cte-source-identity.md), child 1/4 of TASK-78. The immutable proposed case inventory is [v1](task-141-acceptance-freeze.json). **Status: In Progress.** Passing the SQL oracles does not certify source-constructive protocol proof.

## Standards review

The implementation maintains declaration-ordered ordinary CTE bindings, reserves recursive bindings without using inherited physical or column-domain evidence, and tests public Rust analysis and independent DuckDB SQL results. No new dependencies, parser dialect whitelist, lint exclusions, or public JSON contract changes were introduced. Do not assume a scoped CTE has an executable physical producer merely because dependencies and output lineage were resolved.

## Spec review and missing gates

| AC | Evidence available on this PR | Remaining mandatory evidence |
| --- | --- | --- |
| #1 | Frozen v1 source/producer/terminal bags plus v2 56-cell dialect, 12-cell adapter, and conditional release matrix | Independent review of every mandatory and conditional cell |
| #2 | Typed `CteProgram` and `physical_finite_cte_bag_plan` now implement source-complete lexical DAG, row identity, TRUE/FALSE/UNKNOWN, Exact/Impossible/Residual and typed domains | Independent review of all unsupported boundaries |
| #3 | Independent DuckDB source/terminal oracles and public `tests/task_141_physical.rs` assert all four producer bags, identifiers, complete terminal values and counterexamples | Independent review of complete bag oracle equivalence |
| #4 | 14 dialect-name proof equivalence, direct SQL/dbt catalog/ODCS source complete parity and 42 explicit conditional parser/residual classifications added | Independent review of parser/session boundary disposition and native-engine limitations |
| #5 | CI suite runs fmt, strict Clippy, full/default/no-default tests, docs and real dbt E2E | Final PR-head CI and independent review of each mandatory/conditional cell and original parent-AC mapping |

**The bounded mandatory implementation is CI-verified but not independently signed off.** AC #1 through #4 are checked in the task. AC #5, including independent review and the final PR-head merge gate, is open. TASK-141 stays In Progress and TASK-78 stays open.

## Follow-up author review (not independent signoff)

A standards/spec pass identified several situations that could falsely certify a
complete CTE bag when SQL execution has unproved source or binding semantics:

- An `ORDER BY` identifier that does not resolve to a projected output label
  could fail in the engine. The bounded CTE lowering now permits only provable
  projected labels; all exposed parser dialect names are covered by a negative
  public-API test.
- The table-source AST may carry partition selection, time travel, ordinality,
  hints, indexes or a JSON path. These are not interchangeable with one
  complete unmodified source bag. The lowering now requires an ordinary table
  factor, with a MySQL partition-selection regression test.
- A table alias may have positional column renames. Ignoring those aliases
  would borrow physical schema names that SQL no longer exposes. Exact proof
  now excludes that shape, with a public-API regression.

These are soundness fixes within the existing mandatory Exact claims, not new
mandatory acceptance variants. They do not replace a separate independent
review of frozen cases, conditional dispositions and the original parent ACs.

## Second-pass semantic soundness audit

The bounded CTE lowering now treats unquoted names consistently within its
own lexical graph, including case-insensitive local lookup and duplicate WITH
declaration detection. Quoted CTE declarations, quoted source/column references,
and quoted output aliases are explicitly excluded from an Exact physical proof
until dialect-specific name-binding rules are attested. An otherwise valid SQL
text may conservatively return Residual; it cannot borrow a physical table with
the same spelling as a different scoped CTE.

Public API regressions cover quoted local source identity, case-differing
unquoted references with a competing physical table, duplicate WITH names,
and quoted output aliases interacting with ORDER BY. A dialect-specific CTE
FROM extension also cannot silently become a plain projection. The immutable
v1 fixture expectations and four mandatory Exact cases are not weakened by
these safeguards.

The standards/spec review by the implementer is **not independent approval**.
For final TASK-141 signoff, an independent reviewer must inspect all four
mandatory complete-bag cases, 56 dialect proof cells, 12 adapter cells and 42
conditional parser/residual cells, verify the mapped TASK-78 AC #1/#3/#4/#5/#6
without closing TASK-78, and confirm green CI on the final PR head.

## CI and matrix evidence

The reviewed implementation at `bd541cfea112449bd85bf7a5380b8f933f168e02`
passed [all nine CI checks](https://github.com/phdah/sql-semantic-protocol/actions/runs/41141094151):
format, strict lint, default and no-default feature tests, documentation,
dependency boundary, and dbt E2E. The frozen physical checks construct each
mandatory complete bag and assert exact source IDs, producer lineage,
TRUE/FALSE/UNKNOWN, requested Impossible counterexamples and inclusive
integer/NULL domains. Direct, compiled-dbt and equivalent ODCS adapters are
compared against the same physical construction.

All 42 conditional dialect matrix cells now record the exact `parsed`
outcome, verified by calling sqlparser directly with each exposed dialect.
This is **parser** evidence, not a claim of native execution for 13 dialect
names without such an oracle. Only the DuckDB engine has independent
executed source/terminal oracles. Conditional physical proof remains Residual
where source/session/materialization/recursive laws are unproved. The v1
acceptance freeze is intentionally left immutable; the v2 proof matrix and
coverage manifest hold live CI/signoff state.

A reviewer independent of this implementation is still required before
checking AC #5, marking TASK-141 Done or treating the PR as merge-ready.
