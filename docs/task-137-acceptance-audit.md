# TASK-137 finite literal table acceptance audit

**Status: all five TASK-137 child acceptance criteria signed off, pending PR merge.**
The seven mandatory cases, 104 parser paths, 24 adapter dispositions and final
code verification have passed. TASK-77 parent and protocol v3 release remain open.

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

## Child acceptance signoff

| AC | Verified evidence |
| --- | --- |
| 1 | Versioned v1/v2/v3 frozen inventory, both fixture IDs, all exact case outcomes, 104 dialect paths, 24 adapter cells and reviewed conditional dispositions |
| 2 | Canonical rows, typed domains, multiplicity, one-based identities, TRUE/NOT TRUE and Exact/Impossible/Residual paths; overflow and explicit offset aliases fail closed |
| 3 | All seven mandatory complete native DuckDB bags, public physical identities, nullable/bounded domains and impossible negative tuples in `tests/task_137_frozen_matrix.rs` |
| 4 | 69 Exact, 18 parser rejection, 4 lexical boundary, 13 opaque residual dialect cells; compiled dbt model parity, ODCS inapplicability and unexecuted vendor EXPLODE law reviewed |
| 5 | All nine jobs passed on [the verified code-head CI run](https://github.com/phdah/sql-semantic-protocol/actions/runs/40089927405): fmt, strict Clippy, Rust tests, docs, no-default-feature lanes and real dbt E2E. Active schema/golden tests and independent standards/code review are included; final documentation-only head CI is also required before merge |

**TASK-137 is Done only as child scope; TASK-77 parent AC and protocol 3.0.0 release remain open.**

## Observed matrix review (sqlparser 0.58)

The all-cell diagnostic on PR head 4b752b7 observed exactly 104 dialect/variant cases: 69 exact canonical AST proofs, 18 parser syntax rejections, four parsed-but-incompatible bracketed-identifier ASTs (SQLite and MSSQL for UNNEST and WITH ORDINALITY), and 13 deliberately opaque residuals. The v3 freeze records every expected disposition explicitly. The permanent test now checks the parser independently, refuses analyzer errors as parser boundaries, and compares all exact bags, row IDs, domains and positive/negative membership to the frozen DuckDB-validated outcomes.

SQLite explicitly treats square brackets as quoted identifiers. MSSQL also tokenizes the bracketed array-looking expression as an identifier. Neither AST may be promoted to the shared array-literal Exact proof. Typed CAST(array AS INTEGER[]) variants are parser-rejected in nine dialects; none are silently skipped. The 24 adapter cells explicitly distinguish executable SQL/dbt parity from ODCS's lack of source-free literal-row evidence, and record the unbound opaque dbt case as non-evaluable rather than manufactured evidence. EXPLODE retains the source-free canonical operator proof but not native vendor execution certification.

## Independent review: SQL semantics and implementation (2026-10-24)

**Standards and engine axis.** The seven mandatory source-free bags have
independent DuckDB complete-result oracles. Integer bounds are closed at their
attested finite extrema; duplicates retain multiplicity; SQL NULL is a row
value rather than proof of a missing row. Empty and NULL arrays return zero
rows without losing explicit integer element-type evidence. UNNEST ordinal
identities are one-based; a BigQuery OFFSET, when proved, is zero-based.
BigQuery's explicit `WITH OFFSET AS name` syntax has a separate AST alias;
this implementation does not invent a positional column for that unsupported
naming form. The frozen 13-dialect matrix classifies 69 exact parser paths,
18 parser rejections, four bracket-identifier lexical boundaries and
13 opaque residuals. These parser proofs do not certify native SQL execution
on those vendors. EXPLODE native placement/session differences remain an
explicit TASK-137 conditional release boundary, not a hidden pass.
References: DuckDB UNNEST, BigQuery standard SQL query syntax, Databricks
EXPLODE and sqlparser 0.58 AST evidence, linked below.

**Canonical implementation axis.** Analysis normalizes complete literal rows
once, preserving positional identities; the physical API reads the canonical
relation without reparsing SQL, requires a closed source-free layer, and
classifies malformed requested tuples as Residual and well-formed absent
tuples as Impossible. Direct SQL and compiled dbt carry the same typed
wire/table facts, while the 24 adapter cells document the lack of ODCS
source-free row evidence. Additional soundness regressions reject overflow
from explicit narrow signed-integer array casts, preserve in-range casts and
refuse to silently drop a named OFFSET output. These protect the frozen
outcome contract without adding unrelated composition obligations.
TASK-140 owns source-column fanout and composed join/aggregate proofs.

**Closure evidence.** The complete [code-head CI run](https://github.com/phdah/sql-semantic-protocol/actions/runs/40089927405) is green. The final documentation commit must also pass CI before the PR is merged. TASK-77 and v3 release gates remain independent and open.

## Real dbt Core execution oracle

A source-free DuckDB dbt view in `tests/fixtures/dbt_core_project/models/task137_literal_unnest.sql` covers `[1,2,1,NULL]` with stable one-based generator identities. `tests/task_137_dbt_core_e2e.rs` isolates the real dbt-generated manifest and warehouse-introspected catalog, checks the complete physical terminal bag against the materialized DuckDB relation, nullable domain, and impossible absent row. `make dbt-e2e` includes the ignored-by-default executed oracle. Completion requires that CI job to pass on the final head.

## External semantic contract review

The source-free list-law oracle follows DuckDB's documented `UNNEST` behavior, including the zero-output semantics for typed NULL/empty lists: https://duckdb.org/docs/1.4/sql/query_syntax/unnest. The bracketed-identifier AST boundary is grounded in https://www.sqlite.org/lang_keywords.html and https://learn.microsoft.com/en-us/sql/relational-databases/databases/database-identifiers. Spark/Databricks document the one-row-per-array-element `EXPLODE` law, but generator placement changes by engine/runtime version: https://spark.apache.org/docs/latest/sql-ref-functions-builtin.html and https://learn.microsoft.com/en-us/azure/databricks/sql/language-manual/functions/explode. Parser-model normalization and the equivalent DuckDB oracle are not evidence of native vendor execution. TASK-137's conditional release disposition therefore does not certify unexecuted engines.

## Canonical type evidence for empty outputs

Finite relations now emit a positional `column_types` array and expose the same evidence through `PhysicalFiniteTableExact::terminal_column_types`. Typed empty/NULL arrays still advertise their integer element type even with an Empty value domain. All-NULL untyped `VALUES` columns conservatively advertise `unknown`. Tests verify the invariant through direct SQL, compiled dbt, protocol JSON, and dialect parity. This field is part of the active schema/contract, not reconstructed from row values.
