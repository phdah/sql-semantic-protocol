# TASK-150 acceptance audit

**Scope:** TASK-81 child 2/4, independent from the original TASK-81 parent
acceptance and single-v3 release gate. The predecessor TASK-149 is merged.

## Frozen evidence

- `docs/task-150-acceptance-freeze.json` v3 contains **10 finite cases** and 130 individually frozen parser outcomes:
  8 mandatory Exact complete-bag outcomes and 2 conditional typed Residual
  outcomes. Earlier pre-implementation v1 and extended v2 matrices are retained
  as immutable historical references.
- Stable fixtures: `81-C150-P` and `81-C150-N`.
- Exact original source/target rows, selected and NOT TRUE target occurrences,
  affected counts, typed integer-domain frequencies, NULL status and inclusive
  lower/upper endpoints are asserted by `tests/task_150_joined_writes.rs`.
- A bounded read-only `WITH` producer has an exact local complete bag
  traceable to its physical input. The local `picked` bag is included in
  `PhysicalWriteTransition::intermediate_states()`; no independent insertion
  of a named CTE source is permitted.
- `tests/task_150_dialect_adapter.rs` covers 10 cases across 13 parser
  dialects (130 explicit classification paths) and all 10 cases under direct
  SQL/dbt compiled catalog/ODCS schema evidence (30 equivalent adapter cells,
  with ODCS gated by the crate feature).
- Native executable reference is DuckDB, not a native vendor certification.
  In particular UPDATE FROM multiple matching sources is a typed Residual;
  DELETE USING repeats are existential and remove each target occurrence once.
  Missing complete source state is Residual. A joined UPDATE violating an
  enforced primary key is Impossible and rejected by native DuckDB.

## Acceptance mapping

| AC | Required evidence | Disposition |
| --- | --- | --- |
| #1 | Versioned v3 matrix, typed positive/NOT TRUE bag identities, explicit boundaries | Verified on the frozen v3 inventory and CI |
| #2 | Canonical joined-source/CTE write model, complete physical transition, fail-closed ambiguity and key validation | Verified; native executable oracle and full-row domains |
| #3 | Complete native source, CTE intermediate, target after, NULL/duplicate/impossible/rejected contrast | Verified; native DuckDB and full-bag oracles passed final-head CI |
| #4 | All 13 injected parser dialects; direct/dbt/ODCS equivalent schema evidence; conditional native vendor law | Verified; 130 parser cells and 30 adapter cells passed final-head CI |
| #5 | Final head fmt, strict Clippy, default/no-default tests, docs, dbt E2E, schema and golden snapshots | Verified on final signoff SHA `c8efe1e` ([nine green jobs](https://github.com/phdah/sql-semantic-protocol/actions/runs/40918021539)) |

## Two-axis review (Matt Pocock code-review)

**Standards:** The change keeps AST parsing in `src/analysis`, typed canonical
ownership in `src/protocol`, physical proof in `src/physical_realization`
and wire emission in `src/emission`. CTE materialization is shared with
INSERT rather than reimplemented. The public schema/contract and code map are
updated. There are no lint exemptions or unsafe operations. Strict Clippy, rustfmt, public docs, default/no-default-features tests and dbt Core E2E all passed at [implementation CI](https://github.com/phdah/sql-semantic-protocol/actions/runs/40916998581). No new dependencies, unsafe, lint suppressions, or duplicate canonical physical producers were introduced.

**Spec:** Joined UPDATE/DELETE requires a complete, single independent physical
source bag; exact UPDATE requires unique observed source matching per target;
DELETE proves existential row selection without duplicate target deletions.
The bounded CTE producer is constructed from the original source and recorded
in the physical transition. Unproven native vendor semantics and more complex
CTEs remain explicit non-exact boundaries. Parent TASK-81 and release TASK-91
are intentionally not closed.

**Independent review disposition:** Standards: no outstanding violations after correcting legacy regression expectations, rustfmt and canonical API re-exports. Spec: no unchecked mandatory bounded fixture. The original partial CTE no-op bug was fixed, and the bounded local producer has an independently executed full physical witness. Exact SQL/parser behavior is not a claim of native vendor equivalence.

**Verified implementation head:** [CI run 40916998581](https://github.com/phdah/sql-semantic-protocol/actions/runs/40916998581) at SHA `11565f78586ef486ad8c14a66e4993ea406b5ee0` has nine green jobs: fmt, default/no-default Clippy, default/no-default Rust tests and docs, dependency checks and dbt E2E. The 130 exact parser classifications are pinned in the freeze; 30 equivalent adapter cells are independently asserted. The enforced-key Impossible contrast has a native DuckDB rejection test.

**Lifecycle:** All five child acceptance criteria and the independent two-axis code review are verified. Final signoff-head `c8efe1e` passed all nine CI jobs. TASK-150 is marked `Done` **in PR #162** so the merged branch carries the correct task state; PR merge remains the separate integration prerequisite for starting TASK-151. Parent TASK-81 and release TASK-91 remain independently unsigned.
