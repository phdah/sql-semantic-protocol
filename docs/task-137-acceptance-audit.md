# TASK-137 finite literal table acceptance audit

**Status: acceptance open.** PR #144 is a focused implementation and
validation branch; this audit describes its current semantic proof contract,
not certification of all five criteria or a TASK-77 release.

## Acceptance inventory and versioned fixture corrections

The original [immutable v1](task-137-acceptance-freeze-v1.json) and
[corrected v2](task-137-acceptance-freeze-v2.json) remain available.
[Active v3](task-137-acceptance-freeze.json) adds empirically observed dialect and adapter cell dispositions. v2 corrects three demonstrable
mistakes without changing fixture IDs: untyped `[]` cannot claim integer
element evidence, empty-result NULL membership is false (though the nullable
*element type* is separate), and generator identities are one-based positions.
Every mandatory case has exact SQL, source-free full input bag, independent
DuckDB oracle query, expected terminal bag, typed finite domains, stable
row identities and an absent-row goal with Impossible classification.

## Public seams and evidence

- `analyze_sql` / `analyze_inputs`: typed source-free VALUES and
  deterministic array table-normalization without consumer-side SQL parsing.
  Unproven `[]`, `[NULL]` and opaque array expressions remain residual.
- `physical_finite_table_plan`: complete source-free row identities, exact
  terminal multiplicities and domains; malformed requested rows are Residual
  rather than false Impossible. Source-column and composed fanout remain
  unproved instead of inventing physical rows.
- `tests/task_137_frozen_matrix.rs`: seven mandatory positive/NULL/duplicate/
  zero-output cases checked against independent DuckDB SQL and all 104
  dialect/variant parse paths. The EXPLODE native engine law is **not**
  inferred from an executable UNNEST equivalence oracle.
- `tests/task_137_adapter_parity.rs`: direct SQL and compiled dbt v12
  sources feed the same canonical model; asserts output bags and exact JSON.
  ODCS contracts carry *physical relation schemas*, not source-free literal
  row generators. These cases have zero physical source columns, so equivalent
  ODCS evidence does not exist. Do not fabricate an ODCS parity cell.

## Acceptance gates still required

| AC | Required final check |
| --- | --- |
| 1 | Independent reconciliation of every corrected frozen cell, parser and adapter classification, nullable and integer bounds |
| 2 | Validate exact vs residual boundaries for typed empty/all-NULL arrays, one-based ordinality, finite duplicate/absence, and unsupported generator/session cases |
| 3 | Passing final-head DuckDB oracles and public identity/domain/negative membership tests for both fixture IDs |
| 4 | Final 13-dialect accepted/boundary matrix, compiled dbt parity, ODCS inapplicability disposition, and native vendor/session EXPLODE limitation reviewed |
| 5 | Green final-head fmt, strict Clippy, Rust tests, docs, no-default, dbt E2E, schema/golden snapshots and two-axis standards/spec code review |

**No TASK-77 parent criterion or protocol 3.0.0 release is signed off here.**

## Observed matrix review (sqlparser 0.58)

The all-cell diagnostic on PR head 4b752b7 observed exactly 104 dialect/variant cases: 69 exact canonical AST proofs, 18 parser syntax rejections, four parsed-but-incompatible bracketed-identifier ASTs (SQLite and MSSQL for UNNEST and WITH ORDINALITY), and 13 deliberately opaque residuals. The v3 freeze records every expected disposition explicitly. The permanent test now checks the parser independently, refuses analyzer errors as parser boundaries, and compares all exact bags, row IDs, domains and positive/negative membership to the frozen DuckDB-validated outcomes.

SQLite explicitly treats square brackets as quoted identifiers. MSSQL also tokenizes the bracketed array-looking expression as an identifier. Neither AST may be promoted to the shared array-literal Exact proof. Typed CAST(array AS INTEGER[]) variants are parser-rejected in nine dialects; none are silently skipped. The 24 adapter cells explicitly distinguish executable SQL/dbt parity from ODCS's lack of source-free literal-row evidence, and record the unbound opaque dbt case as non-evaluable rather than manufactured evidence. EXPLODE retains the source-free canonical operator proof but not native vendor execution certification.
