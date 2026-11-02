# TASK-160 bounded acceptance audit

**Status:** TASK-160 bounded child signed off on 2026-11-02. TASK-83 remains a separate open parent gate.

## Frozen contract and scope

- Original six-case v1 frozen before code changes in `docs/task-160-acceptance-freeze-v1.json`.
- Explicit v2 addition `83-C160-N-filter-null` corrects a missing AC #3 TRUE / NOT TRUE / SQL UNKNOWN contrast. Original v1 is preserved, and no original mandatory case is weakened.
- Seven v3 cases (v2 archived), including five mandatory Exact/Impossible outcomes and two named conditional Residual boundaries. 13 dialect identities; all 416 individual statement/parser classifications are pinned and verified in the active v3 matrix.
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

## Final verification and independent closure boundaries

- **Mandatory outcome cells:** 5/5 Exact/Impossible, including complete CREATE/INSERT/ALTER/SELECT/DROP, NULL and duplicate occurrences, the SQL three-valued rejected predicate contrast, missing-read impossibility and duplicate ADD contradiction.
- **Conditional cells:** 2/2 explicitly typed Residual (DROP then CREATE generation reset; temporary/vendor/session DDL). No unproved execution law is promoted to Exact.
- **Parser matrix:** All 416 statement-level classifications pinned for 13 dialects in active v3; immutable v1/v2 histories retained.
- **Adapters:** `tests/task_160_adapter_parity.rs` verifies equivalent direct SQL, dbt catalog and ODCS typed schema evidence, including filtered reads. Dbt compiled SQL does not masquerade as executable DDL.
- **DuckDB:** `tests/task_160_ordered_lifecycle.rs` checks each frozen exact case's complete live source, intermediate SELECT/output, catalog and final bags, NULL/bound inclusivity, row counts, and rejected/unknown ordinals.
- **Implementation-head CI:** [41509894079](https://github.com/phdah/sql-semantic-protocol/actions/runs/41509894079), head `fd79c21efe4710178b6f1205d0a07016d4063df1`: 9/9 jobs successful (fmt, all-feature and no-default strict lint, tests, docs, dbt E2E, dependency checks).
- **Standards review:** PASS, zero blockers; canonical ownership, source completeness, deterministic typed domains, fail-closed semantics and no gratuitous JSON contract changes.
- **Spec review:** PASS for all original TASK-160 AC #1..#5 and reviewed v3 matrix. Separate two-axis review [#5742168813](https://github.com/phdah/sql-semantic-protocol/pull/181#pullrequestreview-5742168813), recorded from the PR author's connected account rather than a second human reviewer.

**Child disposition:** Done on the implementation head and review evidence above, with the last metadata-head CI as PR integration verification. **Parent/release boundaries:** TASK-83 original AC #1..#6 and TASK-91 remain open for their own independent closure and cannot be inferred from child CI or an unmerged PR. No sibling task is unblocked until TASK-160 is both Done and merged into `main`.
