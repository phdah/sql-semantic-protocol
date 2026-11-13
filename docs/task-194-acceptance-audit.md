# TASK-194 bounded acceptance audit

Review date: 2026-11-13. Scope: **22 immutable original set SQL cases x 13 project parser dialects = 286 cells**. This report reviews TASK-194 only. TASK-88, TASK-91 and external vendor runtimes are not approved.

## Independent source/terminal evidence

- Original SQL, output shapes, parser observations and explicit negative cases: [task-owned v1 freeze](task-194-acceptance-freeze.json). The freeze is immutable preimplementation evidence, not a postimplementation approval flag.
- [Independent typed SQL tuple review](task-194-independent-bag-review.json) separately enumerates every physical branch occurrence and projects it into each original SQL case, compares whole nullable tuples under UNION/INTERSECT/EXCEPT DISTINCT/ALL, and pins **106 non-emitted candidate occurrences**, 20 native terminal bags, and two explicit negative boundaries.
- `tests/task_194_independent_bags.rs` checks all 20 executable original SQL source and terminal bags against DuckDB, the 106 exact non-emitted candidate occurrences, and nine additional two-column adversarial cases (NULL, duplicate, overlapping cross-column values, BY NAME/positional and nested sets).
- `tests/task_194_native_set_bags.rs` preserves independent original DuckDB goldens. `tests/task_194_physical_by_name.rs` certifies original S083's 7 terminal tuple occurrences, four discarded source duplicates and one provably impossible absent whole tuple across all 13 public parsers, through the actual closed-source `physical_finite_set_tree_plan`. Source-free set witnesses remain Residual.

## Acceptance criteria

| AC | Reviewed evidence | Bounded disposition |
| --- | --- | --- |
| #1 Exact 22x13 v1 inventory and source/negative outcomes | Freeze, independent bag review, `tests/task_194_dialect_semantics.rs`, `tests/task_194_review_ledger.rs` | 22 reviewed SQL cases and all 286 individually classified cells; two named negative syntax/arity cases have no invented terminal bag |
| #2 Tuple counts, NULL, full positive/rejected/impossible and nested/shared sources | `tests/task_194_independent_bags.rs`, `tests/task_194_native_set_bags.rs`, `tests/task_194_physical_by_name.rs` | 20 complete original DuckDB bags, 106 discarded source occurrences, nine adversarial 2-column law cases |
| #3 Canonical full-document/typed output and fail closed | `src/analysis/output.rs`, `src/physical_realization/set_trees.rs`, `tests/task_194_dialect_semantics.rs`, `tests/task_194_physical_by_name.rs` | 260 accepted canonical typed cells; 13 impossible mismatched-arity cells; 13 rejected CORRESPONDING parser cells explicitly unapproved under TASK-191; MINUS/EXCEPT full-document semantics checked for 13 parsers |
| #4 SQL/dbt/ODCS parity and pinned consumer | `tests/task_194_adapter_parity.rs`, [temporary migrated sql-tdg consumer #52](https://github.com/phdah/sql-tdg/pull/52) | Direct/dbt/ODCS source-typed parity for all 20 native originals in 13 parsers; migrated consumer tests on exact pinned candidate are separate from published consumer compatibility |
| #5 Reproducible checks and independent review | `tests/task_194_review_ledger.rs`, [286-cell semantic review ledger](task-194-independent-semantic-review.json), `docs/coverage-manifest.json` | Signoff requires final-head protocol 9/9 CI and pinned migrated consumer 5/5 CI, with links recorded after verification |

## Review partition

There are **260 accepted bounded typed canonical parser cells**, **13 explicit unsupported-arity semantics cells**, and **13 CORRESPONDING parser rejections** that remain *unapproved* and owned by TASK-191. No parser result or DuckDB execution establishes non-DuckDB vendor runtime laws. TASK-179 owns those laws, TASK-197 owns cross-family adapter parity, TASK-180 independently audits the whole matrix. These unapproved *out-of-task* gates must not be represented as TASK-194 successes.

The independent review derives complete tuple algebra from immutable source populations separately from the Rust analyzer and independently executes DuckDB. It is an agent-conducted review, **not** an external maintainer approval. The review concludes on the bounded TASK-194 contract only when the final commit and both source/consumer CI records are linked.
