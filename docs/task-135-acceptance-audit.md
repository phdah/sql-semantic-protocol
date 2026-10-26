# TASK-135 independently bounded acceptance audit

## Scope and status

**Status: pending final-head CI and task signoff.** This child implements the frozen
`76-C135-P` and `76-C135-N` families. TASK-76 parent remains open.
The task-owned immutable matrix is `docs/task-135-acceptance-freeze.json`
(version 2); `docs/coverage-manifest.json` links the tracked owner.
The accepted test matrix contains six mandatory finite SQL/data cells and
three conditional Residual cells. The original TASK-76 AC #2, #4, #5 and #6
are mapped without claiming parent signoff or unexecuted native vendor laws.

## Mandatory fixtures and independent execution evidence

| Frozen case | Semantic and physical proof | Independent oracle |
| --- | --- | --- |
| `76-C135-P-cte-nested` | Inner four rows, outer OFFSET 2 LIMIT 1 selects only `s#2`; terminal `k=3`, inclusive [3,3] | `tests/task_135_nested_pages.rs`; DuckDB source [1,2,3,4,5] and final [3] |
| `76-C135-N-empty` | Complete empty source and terminal; Empty finite value domain, every requested row Impossible | `tests/task_135_nested_pages.rs`, full physical and native empty-bag case |
| `76-C135-P-renamed-producer` | Source `k` -> producer `producer_key` -> terminal `k`; one surviving physical identity `s#2` | `tests/task_135_nested_pages.rs`; direct/dbt/ODCS parity |
| `76-C135-P-set-branch` | Left branch LIMIT 3 before UNION ALL and enclosing LIMIT 3, selected left row0/1/2, page-rejected row3/4, exact terminal [1,2,3] | `tests/task_135_set_branch_pages.rs`, DuckDB full input and output plus requested-4 Impossible |
| `76-C135-N-set-zero` | Source remains five original rows, branch members and rejections complete; outer LIMIT 0 returns Empty | `tests/task_135_set_branch_pages.rs`, native DuckDB zero-result oracle |
| `76-C135-N-null-duplicates` | Two tied `k=1` physical rows with distinct `seq`; explicit NULLS LAST and total secondary order select exactly `s#2,s#3` | `tests/task_135_nested_pages.rs`; SQL/dbt/ODCS complete bags and typed domains |

All finite source/terminal row identities are retained. Ordinary page ties, branch-local WHERE+LIMIT without a certified
WHERE-before-page source law, implicit NULL ordering and an unordered producer
yield typed Residual, never an arbitrary Exact; see `76-C135-R-peer-order`,
`76-C135-R-implicit-null`, and
`76-C135-R-unresolved-producer`. Vendor engine/session equivalence is not
inferred from parser acceptance. All three conditional variants have owner
TASK-135 and reviewed boundary dispositions.

## Dialect and adapter acceptance

The parser/canonical evidence is `tests/task_135_dialect_matrix.rs`: five
SQL shapes over all 13 exposed dialect names (65 individually executed parser
and canonical proof paths, no dialect-gated production logic). Both frozen CTE
source-bag variants share the same SQL shape; their exact physical
positive/empty outcomes are independently exercised in Rust/DuckDB.

Direct SQL, compiled dbt manifest and ODCS source-schema parity:
`tests/task_135_adapter_parity.rs`,
`tests/task_135_set_branch_adapters.rs`, and
`tests/task_135_null_duplicate_adapters.rs`. The authoritative source schema
is supplied by catalog/ODCS as appropriate; the SQL remains identical after
adapter normalization. Actual dbt Core seed/run/docs evidence is
`tests/task_135_dbt_core_e2e.rs` using
`tests/fixtures/dbt_core_project/models/task135_nested_page.sql` and
`seeds/task135_source.csv`, executed by `make dbt-e2e`.
Canonical JSON and schema/golden inventory are regression guarded by
`tests/task_135_acceptance.rs` and the active
`schema/protocol.schema.json`.

## Original and child acceptance reconciliation

| TASK-135 AC | Verification | TASK-76 original |
| --- | --- | --- |
| #1 frozen named positive/negative, domains and statuses | v2 acceptance matrix, six mandatory and three reviewed conditional variants; canonical manifest | #2, #4, #5, #6 |
| #2 source-backed sequential CTE and set-branch proof | `src/analysis/sets.rs`, `src/protocol/{sets,query}.rs`, `src/physical_realization/{page_composition,set_trees}.rs` | #2, #4 |
| #3 independent DuckDB full source, terminal, rejections and Impossible | `task_135_nested_pages.rs`, `task_135_set_branch_pages.rs` | #2, #5 |
| #4 dialect and adapter execution classifications | 13-dialect public matrix, three adapter suites, real dbt Core E2E; native engines explicitly uncertified | #4, #6 |
| #5 code review and final CI | Independent two-axis review, formatting, strict Clippy, Rust tests, docs, no-default-feature jobs, dbt E2E, schema/golden test | #5, #6 |

## Matt Pocock two-axis code review

**Standards axis:** Parsing and SQL AST handling are confined to
`src/analysis/`. Canonical facts are typed and parser-independent,
physical proof reads only canonical semantics and authoritative snapshots,
emission is isolated in `src/emission/`, and library exports are documented.
The existing signed-integer source law, schema validation and canonical
comparison reuse narrow prior code paths. No new crate, unsafe block, lint
suppression or consumer-specific generation logic is introduced. Bounded
sorting/evaluation helpers reflect related direct and set bag semantics, but
the canonical comparison function remains shared; further deduplication is
a nonblocking Fowler duplication heuristic, not a standards violation.

**Spec axis:** Every originally named TASK-135 exact family is represented in
the six-case acceptance inventory, positive and NOT TRUE source identities
are independently exercised, and conservative uncertainties are typed.
Set-branch limits apply before set-operator multiplication; CTE OFFSET does
not leak across a producer boundary. No runtime-native vendor proof or
TASK-76 parent completion is claimed. Final CI is the remaining signoff
gate until the final PR head is verified.

## Release disposition

This PR adds the optional `nested_page` JSON and
`membership.branches[].row_shaping` wire contracts, plus public Rust types
and an additional per-branch page-rejection field. The PR uses a breaking
Conventional Commit title; the next major version is controlled by
Release Please. TASK-76 parent and TASK-91 release gate stay open.
