# TASK-141 acceptance review

Scope: [TASK-141](../.backlog/tasks/task-141%20-%20prove-nested-and-shadowed-cte-source-identity.md), child 1/4 of TASK-78. The immutable proposed case inventory is [v1](task-141-acceptance-freeze.json). **Status: In Progress.** Passing the SQL oracles does not certify source-constructive protocol proof.

## Standards review

The implementation maintains declaration-ordered ordinary CTE bindings, reserves recursive bindings without using inherited physical or column-domain evidence, and tests public Rust analysis and independent DuckDB SQL results. No new dependencies, parser dialect whitelist, lint exclusions, or public JSON contract changes were introduced. Do not assume a scoped CTE has an executable physical producer merely because dependencies and output lineage were resolved.

## Spec review and missing gates

| AC | Evidence available on this PR | Remaining mandatory evidence |
| --- | --- | --- |
| #1 | Versioned fixture identifiers and typed expected SQL bags in the acceptance freeze | Complete frozen dialect/adapter/physical operator cells and independently reviewed boundaries |
| #2 | Lexical relation, output and domain fixes, including nested shadowing and repeated local aliases | Canonical source-complete CTE producer DAG evaluator with exact source/producer/terminal row identities; sound Exact, Impossible and Residual decisions, not SQL-engine-only success |
| #3 | DuckDB SQL source/terminal multiset, identity, domain and three-valued counterexample tests for all four mandatory variants | Compare those independent snapshots to actual public physical-plan results, including every producer bag and rejected/Impossible identity |
| #4 | 14 exposed dialect-name parser paths; direct SQL and dbt compiled identity regressions; equivalent ODCS source schema parity checks | Full typed value-domain and physical proof parity by adapter; documented parser/engine/session disposition on each frozen cell |
| #5 | CI suite runs fmt, strict Clippy, full/default/no-default tests, docs and real dbt E2E | All checks green on final SHA, independent review of each mandatory cell, original parent-AC signoff and approved conditional dispositions |

**No mandatory case is signed off.** The task stays In Progress and PR stays draft until the physical evaluator, adapter proof matrix and independent release-gate review are complete. TASK-78 must remain open.
