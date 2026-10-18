# TASK-115 independent acceptance audit (PR #124)

**Decision: all five bounded portable child acceptance criteria have constructive
evidence.** The frozen source and terminal bags remain unchanged from v1/v2.
Native vendor runtimes and their session/enforcement settings are explicitly
*not* certified. TASK-71 remains open pending TASK-116.

| AC | Independent signoff evidence | Decision |
| --- | --- | --- |
| 1: frozen matrix | `docs/task-115-acceptance-freeze.json` v3: immutable 71-C115-P/N over four variants (eight full source bags), named SQL and AST class, source/column IDs, signed integer and NULL domains with inclusive bounds, selected and NOT TRUE complete Cartesian identity bags, 104 dialect cells, 24 adapter cells, three owned native/session residual boundaries; linked coverage manifest | Verified |
| 2: complete constructive proof | `physical_finite_chain_bag_plan` evaluates the normalized canonical WHERE, two INNER ON equality edges, independent physical source bags, nullable composite keys, enforcement-attested PK/FK and downstream COUNT(*)/HAVING; `PhysicalFiniteChainExact` exposes selected/rejected identities, left filter truth, per-key source contributors, exact terminal bags and value domains. Unsupported operators, right-dependent filter, unproved source collation or enforcement return typed Residual/Impossible instead of Exact | Verified for all mandatory variants |
| 3: independent complete oracles | `tests/task_115_duckdb_matrix.rs` and `tests/task_115_join_filter_group.rs` execute complete immutable DuckDB physical source and terminal bags. `tests/task_115_finite_chain_bags.rs` asserts exact canonical identities, all TRUE/NOT TRUE triplets, per-key contributor provenance, COUNT(*) and inclusive value domains against those immutable fixtures, plus incomplete/contradictory evidence and PK/FK negative tests. `tests/task_115_constraint_oracles.rs` independently checks enforced key/fk insert rejection | Verified for all eight P/N bags |
| 4: dialect and adapter | `tests/task_115_finite_chain_bags.rs` verifies constructive equality against the frozen bag in each of 13 exposed dialects for eight cases (104 canonical cells); all 104 parse as shared SQL, without claiming vendor-native runtime execution. `tests/task_115_finite_adapter_parity.rs` compares complete source identities, selected/rejected triplets, contributor bags, terminal bags and value domains for direct SQL, compiled dbt source/model metadata and equivalent ODCS schema. For PK/FK variants dbt/ODCS declarations retain their canonical constraint facts; without independently attested physical enforcement they return Residual; with explicit enforced source evidence and binary source-ID text law they yield the same exact bag. Three conditional native/vendor law boundaries have reviewed dispositions | Portable verified; native uncertified |
| 5: final-head verification | Rust public API and DuckDB tests, active schema/protocol contract snapshot suite, format check, strict Clippy, all-features/default Rust tests, no-default Rust tests, docs and dbt E2E are all required and exercised in GitHub CI. Initial complete post-implementation success: run [39152508446](https://github.com/phdah/sql-semantic-protocol/actions/runs/39152508446). Final PR head CI must also succeed before merge | Requires successful final-head CI |

## Soundness review

- The WHERE condition is proven to reference only the left source by inspecting
  canonical column instances, never inferred from coincidentally uniform
  observed values. Empty middle/right bags still preserve independent left
  predicate TRUE/NOT TRUE identities.
- Complete input source rows are validated against authoritative typed
  schemas before joining. A source ID is unique within a physical bag.
  Composite ON uses SQL three-valued equality, never NULL= NULL.
- COUNT(*) is derived from the physical identity triples contributing to
  each key. No independent writeable intermediate bag is fabricated.
- Unattested dbt or ODCS constraints remain typed Residual. The explicit
  `AnalysisBundle::attest_source_text_comparison` API accepts only proven
  text equality evidence and retains adapter schema datatypes.
- Dialect parsing and canonical standard semantics are not vendor execution
  guarantees. Non-DuckDB native comparison, nullable/session, and enforcement
  claims stay not certified. Conditional native verification is a separate
  pending *release* consideration, not an unverified mandatory child fixture.

## Ownership and follow-up

All bounded TASK-115 acceptance obligations are child-owned and verified by
independent fixed DuckDB source bags and public canonical assertions.
TASK-116 remains next in the ordered TASK-71 queue **after PR #124 merges**.
TASK-71 and the v3.0.0 release are not signed off by this child audit alone.
