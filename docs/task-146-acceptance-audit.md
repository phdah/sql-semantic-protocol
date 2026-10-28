# TASK-146 independent acceptance audit

Scope: TASK-80 child 2/4, fixtures `80-C146-P` and `80-C146-N`. This
audit maps the complete bounded INSERT SELECT / CTE / target-reorder /
FK / CHECK inventory to original TASK-80 AC #1..#6. Parent TASK-80 and
release gate TASK-91 are independent and remain unsigned.

## Acceptance inventory

The immutable preimplementation v1 and expanded v2 inventories are preserved by Git history. Explicit v3 positional-swapping oracles distinguish SQL output position from source column name even when their values differ. `docs/task-146-acceptance-freeze.json` v3 includes with the previously
missing NULL, FK and ordered-write oracles, independent source row identities,
closed TRUE/NOT TRUE partitions, intermediate CTE bags and typed output bounds.
There are 12 mandatory and three conditional physical cases, 195 parser-dialect
cells, 45 SQL/dbt/ODCS evidence cells, 52 separately frozen vendor
modifier parser-boundary cells and four unsafe scope/ordering boundaries. The tests use the actual frozen SQL
text and declared complete populations, not a consumer-side SQL interpreter.

## Verification by original child criterion

| AC | Evidence | Gate |
| --- | --- | --- |
| #1 Freeze two fixed fixture IDs, full variants, domains, comparison assumptions, source/intermediate/target bags, conditional dispositions | `docs/task-146-acceptance-freeze.json`; `docs/coverage-manifest.json` | Verify every frozen cell |
| #2 Canonical row-local SELECT and one nonrecursive CTE, column reorder, FK/PK/CHECK, ordered poststates, typed fail-closed unproved stages | `src/analysis/writes.rs`, `src/protocol/writes.rs`, `src/physical_realization/write_states/{projection,operators}.rs`; `tests/task_146_insert_select.rs` | Exact/Impossible/Residual reviewed |
| #3 Complete native DuckDB source, local CTE, prior/final target bags, null and duplicate multiplicities, positive and rejected rows, typed inclusive bounds | `tests/task_146_acceptance_matrix.rs`; `tests/task_146_insert_select.rs` | All 12 mandatory case oracles pass |
| #4 Thirteen parser dialect classifications and direct SQL/dbt warehouse catalog/ODCS v3.2 equivalent schema evidence; vendor boundary fail-closed | `tests/task_146_dialect_adapter.rs` | All 195 parser and 45 adapter cells independently checked |
| #5 fmt, strict default/no-default Clippy, full tests, docs, dbt Core E2E, active wire schema, independent standards/spec review | `tests/task_146_insert_select.rs` and GitHub Actions PR #157 | Final-head all 9 jobs green, all mandatory cells reviewed |

## Conditional and vendor execution boundaries

- A missing complete physical source or target is `Residual`. The local
  CTE cannot be passed to the physical-state solver as an independent source.
- Recursive/multiple CTEs, nested unsupported row shaping, and opaque
  comparisons remain explicit `Residual` or canonical unsupported. A
  certified row-local projection must not silently execute an ORDER BY.
- Enforced PK or CHECK contradiction is `Impossible` under the available
  closed-world laws. FK success is Exact with an attested referenced key and
  complete bag; FK missing-reference contradiction remains `Residual`
  without immediate-enforcement or deferral timing evidence, even though
  native DuckDB independently rejects and rolls back that SQL statement.
  Missing enforcement, collation or type proof cannot be promoted to Exact.
- Parser acceptance in 13 sqlparser dialects does **not** certify native
  vendor execution. Only independently executed DuckDB laws are certified.
  TASK-146 owns outstanding native-variant and session evidence; conflict,
  overwrite, returning and upsert belong to TASK-147.
- dbt compiled SELECT models do not execute caller SQL INSERT statements.
  dbt and ODCS parity here attests equivalent warehouse source/target typed
  schema evidence for the same canonical INSERT SQL.

## Final-head release gate

**All TASK-146 child AC #1..#5 are independently signed off against the reviewed v3 portable inventory.** The implementation head `8a5095a91ea2664d2983421c7ba4ecef5ea1036c` passed all nine required jobs in [CI run 40597758597](https://github.com/phdah/sql-semantic-protocol/actions/runs/40597758597), including fmt, strict Clippy (default/no-default), complete Rust tests (default/no-default), documentation, dependency guard and real compiled dbt Core E2E. All 12 mandatory cases (10 Exact, two Impossible) were checked against independent native DuckDB complete physical input/intermediate/output bags. All 195 parser, 45 direct/dbt/ODCS and 52 vendor parser cells were exercised. The three conditional cases and four scope boundaries remain explicitly owned Residual rather than native-vendor certifications.

**Standards review:** A single canonical producer/consumer stage owns row-local SQL semantics, the physical executor never reparses SQL or fabricates local physical input rows, typed null/constraint checks and complete source identities are preserved, and unsupported vendor/session laws fail closed. Optional protocol emission and strict schema, public Rust API and docs agree. No unresolved standards finding blocks the frozen portable child.

**Spec review:** AC #1 fixture/metadata inventory is complete; AC #2 covers direct and bounded CTE SELECT, positional target reorder, ordered poststates and enforced PK/CHECK/FK; AC #3 independently executes positive, negative, duplicates, null, FK/CHECK and full-stage target oracles in DuckDB; AC #4 verifies all 13 parser identities and SQL/dbt/ODCS parity with explicit vendor boundaries; AC #5 has every nine required CI jobs green and active JSON/schema tests. No mandatory gap remains in the reviewed bounded TASK-146 contract.

The final metadata-only PR head must also pass its merge-gate CI. The parent TASK-80, vendor-native execution laws and TASK-91 release approval remain independent and unsigned.
