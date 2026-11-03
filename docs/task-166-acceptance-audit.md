# TASK-166 acceptance and review audit

**Status (2026-11-03):** All five child ACs accepted and TASK-166 Done, with explicit owner waivers for the historical AC #1 freeze error and AC #5 distinct-reviewer identity. Parent TASK-85 remains independently open.

## Frozen contract, correction, and evidence

- The original prospective v1 inventory was committed before semantic edits and is preserved unchanged as [v1](task-166-acceptance-freeze-v1.json). V1 accidentally inverted the SQL TRUE/NOT TRUE source ordinals in the impossible-histogram case and described a **requested**, false terminal bag as if it were the **observed** DuckDB result. That means literal pre-implementation fixture correctness cannot be asserted retrospectively.
- Corrected [v2](task-166-acceptance-freeze.json) retains the original six named cases and fixture IDs `85-C166-P`, `85-C166-N`, explicitly freezes the two aliased SQL terminal shapes, separates the requested goal from the native observed bag, and pins schema/source identity and exact signed-integer/NULL interval domains (with bound inclusivity). Its corrections are versioned; neither the mistake nor its timing is hidden.
- [Machine-readable verification](task-166-acceptance-verification.json) links all **78 case/dialect cells** (52 mandatory, 26 conditional), the native full-bag oracles, typed claims and external-engine restrictions. `tests/task_166_acceptance_inventory.rs` checks each cell, not merely an aggregate count.

## Scope and constructive obligations (AC #2 technical pass)

`src/physical_realization/joint_distributions.rs::shared_filtered_histogram_plan` composes one full, typed `SourceRows` histogram with a directly filtered terminal sharing the same attested physical column. The filter is a normalized exact INTEGER/NULL comparison or NULL test, evaluated against **every** complete-bag bucket under SQL TRUE/NOT TRUE. A returned `WitnessCase` keeps **one** physical population, one closed-world obligation and exact OutputRows for **both** terminals.

A terminal requiring 4 rows from a complete 3-occurrence source, or a requested filtered histogram inconsistent with the exact predicate, is necessarily `Impossible`. The unsupported function and missing source schema are `Residual`, not fabricated rows or guessed engine behavior. Grouping, joins, unassigned columns, non-copy projections and other unproved semantics do not inherit this finite proof. No sql-tdg SQL reparsing is involved.

## Native oracle and emitted evidence (AC #3 technical pass)

| Frozen case | Source | Actually executed terminal A / B | Joint verdict | SQL TRUE / NOT TRUE indices |
| --- | --- | --- | --- | --- |
| `85-C166-P-shared` | [1,1,2] | [1,1,2] / [1,1] | Exact | [0,1] / [2] |
| `85-C166-P-null` | [1,1,NULL] | [1,1,NULL] / [NULL] | Exact | [2] / [0,1] |
| `85-C166-N-upper` | [1,1,2] | [1,1,2] / [1,1] | Impossible (request B=4) | [0,1] / [2] |
| `85-C166-N-histogram` | [1,1,2] | [1,1,2] / [1,1] | Impossible (request B=[2,2]) | [0,1] / [2] |
| `85-C166-C-opaque` | [1,1,2] | [1,1,2] / not executed | Residual | unknown |
| `85-C166-C-missing-schema` | [1,1,2] | [1,1,2] / [1,1] | Residual without typed evidence | [0,1] / [2] |

The acceptance test independently inserts every source occurrence into DuckDB, executes the **entire** source and both representable terminal outputs, checks exact multiset multiplicity, compares SQL TRUE and IS NOT TRUE source ordinals, and validates native INTEGER min/max/NULL/inclusive domain oracles against frozen typed domains. It checks the returned typed public `WitnessDirection` and both requested goals in `to_bundle_json`, including canonical source schema data types and source identities. A jointly feasible proof is not inferred by combining standalone outcomes. The two explicitly impossible targets are **requests**, not claims about native observed output.

## Dialects, adapters, and external boundary (AC #4 technical pass)

- All 13 parser dialect identities are exercised for each frozen case with **two canonical Query statements**, and each case's expected Exact/Impossible/Residual verdict is asserted through the public API.
- `tests/task_166_adapter_parity.rs` compares direct typed SQL, compiled dbt manifest+warehouse catalog, and ODCS physical INTEGER schema under the default feature. It checks a positive shared bag and the necessary B>source impossibility; no adapter invents a physical producer.
- Only DuckDB is a native SQL execution oracle. Parser acceptance for Snowflake, BigQuery, MSSQL, etc. is **not** evidence of native vendor execution or collation/session behavior.
- The opaque predicate and absent type evidence remain named, owner TASK-166, **Residual** with no automatic release promotion. Unexecuted vendor/session laws require an independent versioned attestation.

## CI and downstream consumer evidence

- [9/9 previous standard jobs](https://github.com/phdah/sql-semantic-protocol/actions/runs/41645707286) passed after the native typed-domain differential runner.
- [Full parser/emitted-type verification](https://github.com/phdah/sql-semantic-protocol/actions/runs/41646128619) passed Rust tests, strict Clippy, docs, no-default tests and dbt E2E. Its only failing job was rustfmt; the exact rustfmt patch was committed as `a661009`. A **fully green 9/9 CI run** on final implementation/documentation head `8899a58` is recorded at [CI 41646921872](https://github.com/phdah/sql-semantic-protocol/actions/runs/41646921872). The final reviewed implementation head `be2ab0b` also passed all 9/9 CI jobs ([run 41667049421](https://github.com/phdah/sql-semantic-protocol/actions/runs/41667049421)). Subsequent signoff-only tracking changes require green CI before merge.
- [Pinned sql-tdg compatibility run](https://github.com/phdah/sql-semantic-protocol/actions/runs/41646120167) passed on consumer commit `3beaf7799ab54b2440ae34c917ff7eda6c11c398` with the PR protocol patched in. This covers library, output generation, boundaries, types, full-query and exactness suites. Two preexisting, separately owned timestamp failures were excluded; full sql-tdg green is not claimed. The qualification-only workflow is temporary and must be removed from the final PR.

## Original child criteria and remaining formal decisions

| Child AC | Original parent AC | Result |
| --- | --- | --- |
| #1 prospective exact freeze | #1, #3, #5, #6 | Prospective v1 exists but contained wrong expected ordinal and confused requested/native B bag; v2 corrected and validated. **Owner explicitly accepts the documented historical process deviation and waives literal prospective correctness** on 2026-11-03; AC signed off without rewriting v1. |
| #2 shared source and sound negative proof | #1, #3, #4 | **Technical pass**, 4 mandatory finite cases and conservative conditional boundary |
| #3 full native differential and canonical claims | #5, #6 | **Technical pass**, all executable full-bag/oracle/typed domain checks |
| #4 13 dialect and SQL/dbt/ODCS equivalence | #1, #6 | **Technical pass**, 78 explicit parser+classification cells and conditional evidence |
| #5 final CI and independent review | #5, #6 | **Accepted with owner waiver** of a distinct independent reviewer identity on 2026-11-03. Technical Matt Standards/Spec pass is evidence, not an independent GitHub approval. Reviewed head `be2ab0b` passed 9/9 CI; check final tracking-only CI before merge. |

The owner has explicitly resolved both formal gates, accepted completion, and authorized squash merge of PR #183 on 2026-11-03. A Standards/Spec technical pass submitted through the PR author's own connected GitHub account **does not** constitute a separate reviewer identity, so the owner waiver is explicit and retained. All six frozen cases, 78 parser cells, direct SQL/dbt/ODCS parity, and pinned consumer qualification remain the technical evidence. The two preexisting sql-tdg timestamp failures were excluded rather than claimed fixed. TASK-85 parent remains open until all children merge and its original AC independently pass.

## Formal owner disposition recorded 2026-11-03

The owner explicitly accepted the completed Matt code-review and authorized TASK-166 to be marked Done and PR #183 squash-merged. The owner **waives** (1) historical prospective-v1 freeze correctness, recognizing the unchanged v1 and transparently corrected, exercised v2; and (2) the distinct independent-reviewer identity requirement, while retaining the technical review evidence and not asserting an independent GitHub approval. These are recorded exception dispositions, not claims that the original criteria were literally met without waiver. AC #1 and #5 are closed through owner acceptance. The implementation head `be2ab0b` passed 9/9 CI ([run 41667049421](https://github.com/phdah/sql-semantic-protocol/actions/runs/41667049421)); final tracking-only CI must be green before merge. No native non-DuckDB vendor engine proof or full sql-tdg timestamp suite signoff is implied.
