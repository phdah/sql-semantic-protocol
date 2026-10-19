# TASK-119 independent acceptance audit

## Scope and freeze integrity

- Owner: TASK-119, TASK-72 child 3/4. The parent and the TASK-120 successor are **not** signed off by this PR.
- Finite version 2 freeze: [task-119-acceptance-freeze.json](task-119-acceptance-freeze.json), with 11 uniquely named concrete cases and both `72-C119-P` / `72-C119-N` fixture identities. An independent sign-off iteration repaired an actually nonambiguous tie example, corrected unsupported output domains previously mislabeled **empty**, and added explicit CORRESPONDING, NULL/duplicate and SQL three-valued filtered branches. These are corrections to the task's existing named floor, not a new general SQL promise.
- Five `mandatory_exact` cases: two-column positional aliases, set ORDER BY with explicit NULL order and LIMIT; exhausted OFFSET as exact empty result; FETCH ONLY; a nonempty complete NULL/duplicate bag; and a predicate-filtered bag distinguishing TRUE, FALSE and SQL UNKNOWN.
- Six conditionally classified cases: a LIMIT boundary splitting distinct tuples with tied keys; name alignment (`UNION BY NAME`); vendor `CORRESPONDING`; `MINUS` as canonical EXCEPT where parser-accepted; an unattested cast/coercion; and nested branch-local LIMIT. Unsupported typed domains are **unknown, not empty**. No named conditional grants native engine execution, a type conversion law, or a positional column law without proof.

## AC-by-AC evidence

1. **Frozen matrix:** SQL, physical row identities and snapshots, complete terminal tuple histograms, exact allowed integers/NULL/inclusive bounds, parser and adapter cells and conditional owner/disposition are in the task-owned freeze, linked by `docs/coverage-manifest.json`. The named negative cases cannot be converted to silent success by a future parser upgrade.
2. **Canonical physical behavior:** `src/protocol/sets.rs` stores typed positional sort keys, set tree, and LIMIT/OFFSET/FETCH ONLY without AST types. `src/analysis/sets.rs` normalizes SQL. `src/physical_realization/set_trees.rs` evaluates one full physical bag per unique source, preserves stable row IDs and duplicate frequencies and emits exact terminal value domains only after deterministic shaping. Unknown null-order defaults, unattested name/coercion laws and unsafe branch-local limits stay Residual. A genuine enforced NOT NULL contradiction is Impossible, not Residual.
3. **Independent DuckDB oracles:** `tests/task_119_ordered_set_bags.rs` seeds and **reads back complete DuckDB physical source bags**, executes each mandatory SQL result independently and checks complete terminal tuple histograms. Public API physical plans are separately checked for every source/terminal identity, full allowed-value domains, bounds/NULL, duplicates, SQL TRUE/FALSE/UNKNOWN partitions, exact empty results, Residual and Impossible classifications.
4. **Dialect and adapter parity:** `tests/task_119_dialect_matrix.rs` traverses all 11 frozen cases and all 13 exposed dialect identities (143 classification cells; 65 mandatory shared-syntax cells), checks positional shape and typed ORDER/LIMIT/FETCH laws when mandatory and rejects unjustified positional/vendor laws where conditional. `tests/task_119_set_adapters.rs` compares direct typed SQL, compiled dbt manifest and ODCS evidence for five exact cases (15 parity cells), and additionally checks the three parser-supported conditional residuals in the three adapters (9 fail-closed cells). These are parser and artifact evidence, **not** 13 vendor database execution oracles.
5. **Final-head quality gate:** Every changed source/test/document has to pass Rust formatting, strict Clippy, the full Rust test suite, documentation, no-default-features gates, dbt Core E2E, and schema/golden compatibility checks on the **last PR head**. Do not mark TASK-119 Done until this is confirmed; the merged TASK-118 dependency must remain unchanged.

## Independent standards and spec review

- Separate canonical AST boundary, physical proof and emission; no consumer-specific semantics. No new runtime dependencies, lint suppressions, or `unsafe`.
- Tested strongest safely provable terminal domains, not merely parsing and lineage. Explicit NULL order and tie checks protect against false exactness; unknown coercion and vendor laws stay conditional.
- Frozen mandatory examples are required Exact/Impossible as appropriate; no mandatory residual is disguised as an approved deferral. Native vendor or session-specific behavior and arbitrary casts require a separate evidence law.
- The TASK-72 parent is still open. TASK-120 remains dependent on TASK-119 merging into `main`, not merely on this audit or green child CI.

## Closure

All nine required GitHub Actions jobs passed for the audited implementation on commit `f1dee745`: [CI run 39239403828](https://github.com/phdah/sql-semantic-protocol/actions/runs/39239403828). The 143 dialect classifications, 15 exact SQL/dbt/ODCS adapter cells, 9 conditional residual adapter cells and all five independent DuckDB source/terminal oracles have reviewed dispositions in the frozen matrix. The task is marked Done, while TASK-72 remains In Progress. The closure-only metadata commit must itself pass final-head CI before converting the PR from draft to review-ready; merge is not automated.
