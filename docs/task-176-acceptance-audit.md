# TASK-176 bounded acceptance and two-axis review

**Owner:** TASK-176, TASK-87 child 4/4. **PR:** https://github.com/phdah/sql-semantic-protocol/pull/195. The TASK-87 parent, TASK-91 release gate, and unmerged-sibling dependency rules remain independent.

## Frozen scope and correction

The original preimplementation v1 inventory is retained as `docs/task-176-acceptance-freeze-v1.json`. It fixes fixtures `87-C176-P` and `87-C176-N`, seven mandatory cases, one conditional case, 65 parser cells and 15 adapter cells. **Process deviation:** v1 mistakenly attached the single-column read filter oracle to the separate DML/write case. Active v2 correctly assigns the frozen write family a full two-source `t` and `s` oracle with exact initial/final populations, MERGE/DELETE effects, `v >= 20` bounds, and FALSE/UNKNOWN terminal exclusions. That correction is retrospective and is not represented as complete preimplementation compliance. The original identity, SQL, and family scope remain unchanged. The per-cell signoff lives separately in `docs/task-176-acceptance-verification.json`.

## Child AC mapping

| AC | Evidence and interpretation |
| --- | --- |
| #1 finite acceptance | Original v1 and active corrected v2 matrices, coverage manifest entry, 8 case IDs, 65 dialect and 15 adapter cells; v1 write-oracle defect explicitly recorded as a process exception |
| #2 exact outcomes | `tests/task_176_adapter_write_parity.rs` checks table, view, ephemeral, incremental declaration, source-independent full-bag read outcomes, compiled dbt source identities, canonical MERGE/DELETE write effects, typed constraints, and non-invention of dbt execution |
| #3 independent physical oracles | Fresh DuckDB connections compare complete raw and derived table/view/CTE bags, physical NULL/duplicate/TRUE/NOT TRUE occurrences, mixed DML target and terminal snapshots, as well as impossible COMMIT and residual concurrent isolation |
| #4 dialects and adapters | Every frozen dialect cell is executed against the advertised parser and canonical wire write shape; every adapter cell is compared with equivalent typed direct SQL / dbt / ODCS evidence, while default-disabled ODCS is explicitly not executed under `--no-default-features`. Native execution beyond DuckDB is not asserted |
| #5 final-head release integration | Repository CI must pass fmt, strict Clippy, Rust full and no-default tests/docs, dependency tree and dbt Core E2E. Existing schema/golden assertions are exercised by the full test suite. The pinned `phdah/sql-tdg` depends on published protocol 2.0.3 without default features; its tests cannot attest unreleased TASK-176 dbt/ODCS code. No runtime/wire change is introduced here, and the v3 consumer gate stays under TASK-91. A two-axis review must be recorded on the final head before bounded child signoff |

## Named boundaries

- **Exact:** identical physical source rows `[1,1,NULL,0]` produce `[1,1]` with TRUE ordinals `[0,1]` and NOT TRUE ordinals `[2,3]` (FALSE 3, UNKNOWN 2); the `k >= 1` output domain has a closed lower bound 1 and excludes NULL. In the write script, `t=[(1,10),(2,NULL),(3,99),(4,10),(5,NULL)]`, `s=[(1,30)]`, after MERGE and DELETE yield `[(1,30),(3,99),(4,10),(5,NULL)]` and terminal `[(1,30),(3,99)]`, with `v >= 20` inclusive.
- **Impossible:** bare COMMIT without an active transaction; independent DuckDB rejects it.
- **Residual:** unattested concurrent isolation and vendor-specific runtime, materialization, and enforcement laws. Source metadata cannot establish an executed incremental MERGE, persisted view/table materialization or verified constraint enforcement.
- **Conditional:** other vendors' parser acceptance is canonical syntax evidence only. No native vendor execution guarantee is inferred; TASK-176 owns the documented limitation, and TASK-91 owns release-facing consumer proof.

## Two-axis review

**Standards axis:** No production model, wire schema, new dependencies, unsafe blocks or lint exemptions change. The tests use existing public adapter, typed domain and physical-proof APIs. The task-owned matrix and documentation maintain deterministic source identities and preserve adapter-specific provenance instead of inventing equivalence of authority.

**Spec axis:** Three paired adapter families plus explicit write and negative evidence are exercised at the public API. Native DuckDB snapshots are independent of protocol calculations; the write oracle and NULL/FALSE/UNKNOWN cases check the full bag and typed terminal bound. The adapter/runtime boundary remains sound. The v1 prefreeze defect is disclosed, not erased.

**Review independence:** A review submitted by the PR author's connected account is a same-account two-axis check, **not** independent second-human peer review. Technical owner-directed signoff may transparently record this exception, but does not waive branch protection or team requirements.

## Integration state

The implementation-head CI completed 9/9 jobs at `52f65e8674213c53d1ce1c20cb3ebc4cbe308d45`: https://github.com/phdah/sql-semantic-protocol/actions/runs/42397057115. A standards/spec two-axis same-account review is recorded at https://github.com/phdah/sql-semantic-protocol/pull/195#pullrequestreview-5814037915. The child is technically signed off with disclosed freeze and independent-human review exceptions; the final metadata-head CI remains an integration check. TASK-87 and TASK-91 are not closed.
