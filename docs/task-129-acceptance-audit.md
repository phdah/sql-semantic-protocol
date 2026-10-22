# TASK-129 finite EXISTS / NOT EXISTS acceptance audit

**Task:** [TASK-129](../.backlog/tasks/task-129%20-%20prove-exists-and-not-exists-correlated-uncorrelated.md), child 1 of TASK-75.  
**PR:** [#140](https://github.com/phdah/sql-semantic-protocol/pull/140) (unmerged).  
**Freeze:** [v1 acceptance cases](task-129-acceptance-freeze.json), registered in `docs/coverage-manifest.json`.

## Scope and physical law

The finite freeze has exactly six mandatory cases, grouped under immutable `75-C129-P` and `75-C129-N` IDs:
- Correlated EXISTS with two distinct matching inner *physical* rows: only outer `o1` survives. Outer `o2` and `on` (NULL key) are SQL NOT TRUE.
- Correlated NOT EXISTS on the **same source bags**: `o2` and `on` survive, `o1` is rejected.
- Non-correlated EXISTS/NOT EXISTS with nonempty nested bag: all/none of the outer rows survive, respectively, with three explicit physical inner instances.
- Non-correlated EXISTS/NOT EXISTS with **empty** nested bag: none/all of the outer rows survive, respectively.

Each case fixes exact SQL, physical `id` and `k` source columns, full typed source bags, SQL source aliases, the complete per-outer inner matching witness bag, selected/rejected outer ID lists, complete projected terminal values and inclusive integer bounds/NULL disposition. There is no independent-column-histogram shortcut, invented row or open-world negative proof.

## Canonical executable evidence

- `src/subquery_witness.rs` already normalizes supported EXISTS/NOT EXISTS directions. `src/physical_realization/exists_bags.rs` is their independent finite closed-world interpretation through existing schema validation and the canonical source graph.
- `tests/task_129_exists_bags.rs`: public canonical proof and all required 13 parser dialect identities x six cases, frozen terminal/column/row-instance proof assertions, independent DuckDB source/terminal bag execution and NOT TRUE complement assertion, unknown-filter/syntax fail-closed tests, and duplicate-row-identity impossibility.
- `tests/task_129_adapter_parity.rs`: six equivalent cases through direct SQL, compiled dbt v12 source schema and ODCS v3.2 source contracts. It compares all six proof components for every adapter on one unchanged complete physical source snapshot.
- `docs/code-map.md` documents canonical ownership. No emitted protocol JSON fields were added or reinterpreted; the physical proof is a new public Rust API. Therefore the active protocol JSON schema requires no change.

## Conditional claims and boundaries

`TASK-129` owns non-equality correlations, additional nested predicates, shaped subqueries, unknown scalar domains and vendor/session runtime semantics. These are **Residual**, not false `Exact` or a claim that all vendor SQL engines were executed. Only 13 parser/canonical classifications and independent DuckDB execution can be certified on this PR; vendor-native runtime equality/coercion/collation would require a separate vendor-engine oracle and release review. Parent TASK-75 and protocol v3 gates remain open.

## Acceptance and CI signoff

- [x] AC1: immutable six-case typed source/terminal matrix, all six variants and 13 dialect/three adapter obligations independently verified and updated in freeze
- [x] AC2: full physical exact/NULL/duplicate/multiplicity/Impossible/Residual semantics checked against independent expected bags
- [x] AC3: complete DuckDB source/selected/rejected/terminal and individual row identity oracles verified
- [x] AC4: 78 parser/canonical cells, 18 adapter parity cells and explicit conditional native/session review verified
- [x] AC5: final-head format, strict Clippy, Rust tests with/without defaults, docs, dbt E2E and active schema/snapshot CI all green, independent review completed

## Independent acceptance review and CI

All six frozen mandatory physical source/output cases passed the complete public API and independent DuckDB oracles, including explicit NOT TRUE complements. The six-case x 13-dialect canonical matrix proves 78 parser-equivalent cells, and six cases x direct SQL/dbt/ODCS proves 18 adapter parity cells. Every source and output has stable physical instance IDs and exact nullable integer bounds; missing evidence and forbidden native/vendor claims remain Residual.

- **Standards review (Matt code-review axis):** physical laws live in the canonical Rust proof module; adapter paths only supply equivalent schemas; unsupported predicates and output shapes fail closed. No consumer SQL AST reparsing, uncontrolled input sampling, or false native engine certification. Public API and task-owned oracle fixtures are documented. No outstanding documented-standard findings in this bounded diff.
- **Spec review (Matt code-review axis):** all four named variants, six frozen P/N cases and all five child AC are covered; duplicate and NULL inner rows and empty physical sources yield the independent SQL oracles, including per-outer closed-world absence. No unverified mandatory cells or identified scope additions.
- **Final semantic-head CI:** [run 39791120871](https://github.com/phdah/sql-semantic-protocol/actions/runs/39791120871), **all nine jobs successful**: format, strict default/no-default Clippy, Rust default/no-default tests, docs, no-default dependency check and dbt Core E2E. Active snapshots and schema regressions run under the full test suite.

**Decision:** All five bounded TASK-129 AC verified for the portable finite scope. TASK-129 Done is a child signoff, **not** TASK-75 parent or 3.0.0 release approval. PR #140 must merge before dependency TASK-130 may begin; vendor native/runtime/session behavior remains explicitly uncertified.
