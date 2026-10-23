# TASK-137 finite literal table acceptance audit

**Status: implementation in progress; child AC #1..#5 not signed off.**
This audit describes the current bounded implementation without implying
release or native vendor certification.

The v1 acceptance freeze at [docs/task-137-acceptance-freeze.json](task-137-acceptance-freeze.json)
owns positive/rejected fixture IDs `77-C137-P` and `77-C137-N`, 13 named
parser dialects, adapter obligations and the fail-closed boundaries.

The canonical model `FiniteTable` retains exact literal value rows,
multiplicity and producer identities for supported source-free `VALUES`,
literal `UNNEST` and `EXPLODE` forms. The physical prover returns exact
terminal bags/domains, or impossibility for a requested absent row.
Unattested arbitrary arrays, nested source columns, joins and table functions
are not certified.

Public focused tests: `tests/task_137_finite_tables.rs` uses independent
DuckDB source-free SQL bags, NULL, duplicates, empty typed array and
ordinality, plus negative membership and parser regressions.

## Remaining signoff gates

- Run all focused Rust tests; reconcile parser boundaries with the 13-dialect
  inventory, including precise UNNEST and EXPLODE dialect support.
- Execute and check equivalence of direct SQL, compiled dbt and ODCS catalog
  adapter paths; freeze concrete fixture and source identity evidence.
- Confirm distinct NULL/empty array behavior on available native engines.
- Run fmt, strict Clippy, all Rust tests, docs, no-default features, dbt E2E,
  schema/golden fixtures on the final PR head. Review every mandatory cell.
- Only then check TASK-137 criteria, update coverage manifest and treat child
  as Done. TASK-77 remains separately open.
