# TASK-174 implementation acceptance and review audit (2026-11-07)

**Scope:** TASK-174 child 2/4 of TASK-87. The frozen v1 acceptance file remains unchanged; this separate verification ledger records observed results. TASK-87 remains To Do and protocol 3.0.0 is not approved by this child.

## Evidence and CI

- Acceptance freeze: [task-174-acceptance-freeze.json](task-174-acceptance-freeze.json), 7 named cases (5 mandatory, 2 conditional), 65 case/dialect cells, 20 case/adapter cells.
- Verification: [task-174-acceptance-verification.json](task-174-acceptance-verification.json), complete per-cell classifications.
- Public API tests: [tests/task_174_materializations.rs](../tests/task_174_materializations.rs).
- Verified implementation code head: `9b33de4c17abca831416ca84dd3fd5e90e7fce64`; [GitHub Actions run 42370737926](https://github.com/phdah/sql-semantic-protocol/actions/runs/42370737926) passed **9/9** (fmt, strict lint, tests and docs with/without default features, no-default dependency check, dbt Core E2E); active schema and golden tests are included in the full test suites.
- Oracle: complete `raw.orders = [1,1,NULL,0]` with ordinal identities; TRUE occurrences #0/#1, FALSE #3, UNKNOWN #2; terminal `[1,1]` including duplicate preservation; typed lower bound `>=1` inclusive. Independent DuckDB checks agree with the complete protocol source and terminal bags for SELECT, table/view and inline query.

## Child AC disposition

| AC | Review finding |
| --- | --- |
| #1 Freeze | PASS. v1 freeze predates semantics and remains immutable. All 5 mandatory and both conditional variant IDs, 65 case/dialect cells and 20 case/adapter cells are represented and independently recorded in the verification ledger. |
| #2 Semantics | PASS within frozen bounded scope. dbt manifest materialization is separate from model SQL and `execution: unattested`. View, table, ephemeral and incremental retain the canonical typed SELECT outcome; no incremental physical target is manufactured. |
| #3 Full-bag execution | PASS. Public-API tests compare typed protocol domains, complete source and terminal bags, NULL/FALSE rejection, duplicates and source ordinals with real DuckDB queries. The compiled ephemeral consumer is executed against physical `raw.orders` without creating an ephemeral relation. |
| #4 Dialect/adapter parity | PASS within parser/evidence boundaries. All 65 frozen SQL shapes are individually checked. The generated `__dbt__cte__` name has two explicit parser boundaries (ANSI and Hive); these are not native-engine regression claims. All 20 dbt manifest/catalog/direct SQL/ODCS cells are checked for representable facts. |
| #5 CI and review | PASS for child-owned code, complete schema/golden and dbt E2E. This is an owner-directed, two-axis same-account review exception, not an independent second human approval. The new protocol commit is unreleased; consumer candidate integration is a separate release gate, not misreported as verified. |

## Conditional and cross-repo boundaries

- **Compiled ephemeral dependencies:** dbt's compiled CTE SQL is analyzable, but the manifest alone does not independently prove the compiled expansion equals the referenced ephemeral model. `inline_dependencies: unattested` persists. Without compiled SQL containing Jinja, the adapter explicitly fails closed.
- **Incremental model target:** `config.materialized = incremental` declares intent. No `run_results`, target prestate, execution plan, incremental strategy, target poststate or engine session law is attested. Target write feasibility is **Residual**, not an executed MERGE/INSERT. Owner TASK-174; native verification required for a stronger guarantee.
- **Vendor/session laws:** Parser acceptance is not execution validation. The only executed native oracle is pinned DuckDB `~1.10506.0`. Other engines need pinned version/session, physically observed source and terminal bags and NULL/duplicate/counterexample oracles before certification. Owner TASK-174; retain Residual for unproved native execution.
- **Parser dialect boundaries:** ANSI and Hive sqlparser reject the dbt-style generated CTE identifier starting `__dbt__`. The v1 matrix permits named parser boundaries; both are recorded and reviewed rather than silently skipped. This does not permit a claim that a dbt adapter for either engine executed successfully.
- **Consumer release gate:** Current sql-tdg `Cargo.toml` pins published `sql-semantic-protocol = 2.0.3`. Its last main CI was green, but no test against this unreleased, new-root-field candidate ran. This is **not** a consumer compatibility signoff: sql-tdg TASK-43 and protocol TASK-90/91 own candidate integration and release approval.

## Review provenance and parent mapping

**Standards review:** PASS. Parser/analysis/emission boundaries remain separated; no dependencies or unsafe code, no lint suppressions, deterministic ordered evidence, publicly documented type and schema, and explicit unsupported/error paths. The optional emitted `producer_evidence` is release-facing and subject to the planned 3.0.0 integration gate.

**Spec review:** PASS for five mandatory bounded variants, two explicit failure variants, 65 parser cells, and 20 adapter cells. There is no claim of unexecuted warehouse state. Source identity, nullable source, inclusive output bound, complete multiplicity and NOT TRUE source partition are recorded.

**Review provenance:** Same connected author account conducted a separate standards/spec audit under the owner's direction. This is an owner-directed review-process exception, **not an independent second-person review**. Parent TASK-87 original AC #1, #2, #3, #5, #6 mapping reviewed only for TASK-174's bounded contribution. Parent ACs remain unchecked until TASK-175/TASK-176 and parent closure pass.

**Disposition:** TASK-174 child complete and ready for PR integration, subject to final-head CI; TASK-87 and 3.0.0 release remain open.
