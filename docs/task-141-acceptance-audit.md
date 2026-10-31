# TASK-141 acceptance review

Scope: [TASK-141](../.backlog/tasks/task-141%20-%20prove-nested-and-shadowed-cte-source-identity.md), child 1/4 of TASK-78. The immutable proposed case inventory is [v1](task-141-acceptance-freeze.json). **Status: Done (pending PR merge).** Passing the SQL oracles does not certify source-constructive protocol proof.

## Standards review

The implementation maintains declaration-ordered ordinary CTE bindings, reserves recursive bindings without using inherited physical or column-domain evidence, and tests public Rust analysis and independent DuckDB SQL results. No new dependencies, parser dialect whitelist, lint exclusions, or public JSON contract changes were introduced. Do not assume a scoped CTE has an executable physical producer merely because dependencies and output lineage were resolved.

## Independent standards and spec review (2026-10-31)

**Standards:** Reviewed AGENTS.md, CONTRIBUTING.md, the focused code map and Fowler smell baseline against the full PR changes. Two fail-open dialect cases were identified and fixed: ClickHouse `ORDER BY ... WITH FILL` synthesizes rows and `GLOBAL JOIN` has unproved distributed execution laws. Both now return `Residual` via the public physical-proof API, with targeted regression tests in `tests/task_141_physical.rs`. No remaining blocking documented-standard violations were found.

**Spec:** Reviewed all four immutable mandatory cases against the canonical producer graph, complete physical bags, DuckDB source/producer/terminal oracles, typed NULL/inclusive bounds, positive and Impossible membership, and rejected FALSE/UNKNOWN pairs. Cross-checked the 56 canonical parser/dialect proof cells, 12 direct SQL/dbt compiled+catalog/ODCS adapter cells, and 42 explicitly classified conditional parser/residual cells. The canonical proof is not misrepresented as 13 additional native engine executions: only DuckDB supplies an independent engine oracle. The original TASK-78 AC #1/#3/#4/#5/#6 mapping is satisfied for the bounded TASK-141 scope; the remaining parent scope belongs to TASK-142/143/144, so TASK-78 stays open.

**Signoff:** AC #1 through #5 are reviewed and satisfied for TASK-141, supported by the final semantic-code CI run at [`cb4d257`](https://github.com/phdah/sql-semantic-protocol/actions/runs/41182159741). TASK-141 is Done in this PR, pending integration. The PR merge still requires green checks on its final head. This review does not close the parent or approve the overall 3.0 release.

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

The preceding author reviews are historical. The separate final two-axis review above covers the remaining independent signoff gate and the ClickHouse edge-case fixes.

## CI and matrix evidence

The reviewed semantic implementation at `cb4d257db75ec36bbc89ee50171196f395a679a6` passed [the nine required CI checks](https://github.com/phdah/sql-semantic-protocol/actions/runs/41182159741):
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

Independent standards/spec review is recorded above; TASK-141 is marked Done in this branch. Final-head CI remains the GitHub merge gate.
