# TASK-160 bounded acceptance audit

**Status:** Implementation under verification, not signed off. TASK-83 remains a separate parent gate.

## Frozen contract and scope

- Original six-case v1 frozen before code changes in `docs/task-160-acceptance-freeze-v1.json`.
- Explicit v2 addition `83-C160-N-filter-null` corrects a missing AC #3 TRUE / NOT TRUE / SQL UNKNOWN contrast. Original v1 is preserved, and no original mandatory case is weakened.
- Seven v2 cases, including five mandatory Exact/Impossible outcomes and two named conditional Residual boundaries. 13 dialect identities; 416 individual statement/parser classifications are the scope floor.
- Original TASK-160 AC #1..#5 and TASK-83 parent AC #1..#6 remain independently traceable in the matrix.

## Semantic implementation and evidence

| Obligation | Contract owner | Verification |
| --- | --- | --- |
| CREATE empty / INSERT / ALTER ADD DEFAULT / SELECT / DROP | `src/physical_realization/lifecycle_scripts.rs` and canonical write interpreter | `tests/task_160_ordered_lifecycle.rs`: live catalogs, typed schemas, full bags, counts, SELECT output |
| SQL NULL and duplicate occurrences | Canonical physical rows and source ordinals | Frozen positive and NULL variants, native DuckDB source and terminal bags |
| SQL WHERE TRUE and NOT TRUE (including UNKNOWN) | Existing canonical scalar SELECT evaluator | Frozen predicate-rejection variant and independent DuckDB row oracle |
| DROP before read and duplicate ADD contradiction | Canonical physical write-state proof | Impossible classification plus DuckDB error/untouched prestate |
| DROP -> CREATE generation and temporary/session DDL | Unsupported/conditional boundary | Residual; no invented generation or vendor transaction law |
| SQL dialect parser and canonical AST boundaries | `tests/task_160_dialect_matrix.rs` | 13 dialects x all frozen statements; each parser/canonical cell must be pinned before signoff |
| Direct SQL, dbt catalog and ODCS schema parity | `tests/task_160_adapter_parity.rs` | Equivalent schema evidence must yield the same complete read and catalog; compiled dbt model DDL explicitly rejected |
| Public API / protocol contract | `src/lib.rs`, `docs/protocol.md`, `docs/code-map.md` | Additive Rust API, no JSON protocol wire change |

## No false exactness

The new read API never treats a missing population as a physically absent relation, never derives a stored table from CTE or view metadata, never invents a post-ALTER column datatype, and never accepts a computed/non-integer expression without a typed projection proof. Any unproven source and vendor session law remains Residual. A witnessed DROP followed by a SELECT of that target is Impossible.

## Remaining gates before child Done

1. Freeze exact per-dialect parser/canonical classifications for all 416 v2 statement cells and reconcile any invalid assumptions.
2. Verify the final-head Rust fmt, strict Clippy, tests, docs, no-default-features and dbt E2E CI.
3. Resolve any final direct SQL/dbt/ODCS parity and native DuckDB failures.
4. Perform separate Standards and Spec code-review passes, correct findings, and record the reviewed CI head.
5. Mark TASK-160 Done only after all five original AC and mandatory cells pass. Keep TASK-83 open until independently certified.

No parent TASK-83 acceptance criterion is automatically signed off by this child.
