# TASK-130 independent acceptance audit

**Owner:** TASK-130 (TASK-75 child 2/4). **PR:** [#141](https://github.com/phdah/sql-semantic-protocol/pull/141).
**Acceptance freeze:** [task-130-acceptance-freeze.json](task-130-acceptance-freeze.json), version 2.
**Status:** all five finite TASK-130 child acceptance criteria independently verified on implementation head. TASK-75 parent signoff and protocol release remain separate. Child PR remains unmerged.

## Frozen mandatory inventory

- **15 source-complete cases:** six scalar and nine two-column tuple cases. Fixtures `75-C130-P` and `75-C130-N` each contain explicit TRUE and SQL NOT TRUE row identity partitions. Both normal and correlated forms of IN and NOT IN, duplicate candidates, empty candidate sets, outer/inner NULL, definite unequal tuple fields, and NULL-sensitive negation are included.
- **195 parser/canonical obligations:** 15 cases × 13 exposed sqlparser dialect names. `tests/task_130_in_bags.rs` executes the full source-backed physical proof for each, asserting exact selected/rejected row IDs, all candidate row IDs, SQL three-valued result per outer row, complete terminal bag, signed integer domain values, NULL status and inclusive bounds.
- **45 source-adapter obligations:** 15 cases × direct SQL, dbt compiled manifest with authoritative source column metadata, and ODCS v3.2 with equivalent physical source evidence. `tests/task_130_adapter_parity.rs` independently compares complete canonical physical proof values for these three sources.
- **15 independent DuckDB oracles:** `tests/task_130_in_bags.rs` loads full physical sources and obtains terminal and per-row truth independently with SQL. Scalar IN/NOT IN execute natively. Tuple cases execute the standard fieldwise conjunction with native `AND`, `IS TRUE`, `IS NULL` and closed-world `EXISTS` to certify standard SQL row equality, not Rust's evaluation implementation.

## Boundaries and fail-closed safety

- The canonical operand is `InSubqueryOperand::Scalar` or `InSubqueryOperand::Tuple(InSubqueryTuple)`; the latter validates arity `>= 2`. Every tuple position is retained in `SubqueryMembershipWitness::membership_keys()` and emitted with the ordered `membership_keys` field. The scalar-only directional witness obligations are Residual for tuples; complete-bag physical evidence separately certifies Exact. The older scalar-only constructive shortcut refuses tuples instead of claiming it has proven them.
- The physical proof requires exactly one direct outer and inner source, attested signed integer columns for all keys and correlations, one normalized equality correlation at most, complete schemas, unique source row identities, and a transparent one-column output projection. DISTINCT inner candidates are Residual because they change the complete candidate-row identity bag. Other nested shapes, mixed/noninteger collations and insufficient physical evidence are typed `Residual`; inconsistent source IDs or constraints are `Impossible`.
- **Native-engine conditional:** DuckDB's raw tuple `IN` with NULLs sometimes returns UNKNOWN where fieldwise SQL row equality is FALSE because of a different non-NULL component. Therefore the normative portable source/terminal oracle uses independent expanded SQL comparisons, while the native raw DuckDB tuple `IN` law is explicitly uncertified. Other vendor/session engine laws likewise remain conditional. No claim of cross-vendor native equivalence is made.
- The complete candidate bag is validated for each outer identity, with no inference from scalar marginal histograms or invented intermediate rows.

## Independent standards and spec review

**Standards axis:** Model owns canonical SQL semantics; emitter contains serialization only; physical realization owns closed-world execution; adapters translate only source/schema evidence; deterministic ordered maps and explicit exactness boundaries; no `unsafe`, no new dependency, no consumer-specific SQL logic. Review implementation APIs and non-scalar constructive paths for accidental scalar promotions.

**Spec axis:** All five child AC are mapped directly to the frozen matrix, executable public boundary tests, adapter parity and final CI. The parent TASK-75 AC remain unchecked. Conditional vendor laws are not an excuse for a missing portable mandatory tuple case.

## Final-head signoff gate

- [x] `cargo fmt --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test`
- [x] `cargo doc --no-deps`
- [x] no-default-features tests/docs/lint/dependencies
- [x] dbt Core E2E, schema/golden snapshots
- [x] audit every frozen mandatory row and original AC mapping, with no unchecked required cells
- [x] record final semantic-head CI link and reviewed residual dispositions

**Final semantic implementation head:** `985d284162334c79d0e512d56913fbde5cc0705e`, [CI evidence](https://github.com/phdah/sql-semantic-protocol/actions/runs/39821243913), all nine jobs successful. The code and public contract were independently reviewed against the five frozen child AC, including 195 parser/canonical and 45 adapter cells. No mandatory cell remains unchecked; native DuckDB NULL-tuple comparison, non-integer comparisons and non-DuckDB vendor/session laws remain conditional, with reviewed release dispositions. Closure record modifies documentation/metadata only; final PR-head CI still applies.
