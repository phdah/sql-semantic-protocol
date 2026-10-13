# TASK-100: TASK-68 bounded physical-source composition sign-off

This certifies the original TASK-68 acceptance criteria for the documented
**bounded constructive and typed fail-closed protocol scope**. It does not
approve protocol 3.0.0, unverified feature/dialect variants or sql-tdg's
generator. The authoritative source construction result remains typed
`Feasible`, `Impossible` or `Residual`. A successful DuckDB example
by itself never upgrades a general residual to a constructive proof.

## Preconditions and evidence ownership

The dependency-ordered TASK-93..99 backlog files are Done and their
implementations are merged into `main`:

| Task | Merged PR | Canonical owner | Evidence |
| --- | --- | --- | --- |
| 93 | [#93](https://github.com/phdah/sql-semantic-protocol/pull/93) | `src/physical_realization/graph.rs`, `joint_counts.rs` | `tests/physical_realization/joint_graph.rs`, `joint_filters.rs` |
| 94 | [#94](https://github.com/phdah/sql-semantic-protocol/pull/94) | `src/physical_realization/joins.rs` | `tests/physical_realization/joins.rs`, `joint_operators.rs` |
| 95 | [#95](https://github.com/phdah/sql-semantic-protocol/pull/95), [#101](https://github.com/phdah/sql-semantic-protocol/pull/101) | `src/physical_realization/{composition,grouping,ranking,sets,membership}.rs` | `tests/physical_realization/{mixed,joint_operators,group_rank_set,nested_sets}.rs` |
| 96 | [#102](https://github.com/phdah/sql-semantic-protocol/pull/102) | `src/physical_realization/{distributions,joint_distributions}.rs` | `tests/physical_realization/{distributions,joint_distributions,lineage}.rs` |
| 97 | [#103](https://github.com/phdah/sql-semantic-protocol/pull/103) | `src/physical_realization/{counts,joint_counts,filters}.rs` | `tests/physical_realization/{negative_cardinality,joint_filters}.rs` |
| 98 | [#104](https://github.com/phdah/sql-semantic-protocol/pull/104) | `src/physical_realization/write_states.rs`, `write_states/` | `tests/physical_write_states.rs`, `physical_write_adapter_parity.rs` |
| 99 | [#105](https://github.com/phdah/sql-semantic-protocol/pull/105) | protocol-owned `tests/fixtures/conformance/v1/suite.json` | `tests/protocol_conformance_{physical,dialect_laws,dbt_e2e}.rs` |

The original physical reference DAG starts in merged PR
[#92](https://github.com/phdah/sql-semantic-protocol/pull/92).
Backlog status is never a substitute for the proof tests below.

## Original parent acceptance-criteria traceability

| TASK-68 AC | Implementation and safety argument | Checks and public contract |
| --- | --- | --- |
| **#1** Physical identity, truth, lineage, multiplicity and witness composition | `PhysicalPlanRef` deduplicates controlled leaves; `physical_joint_source_plan`, `physical_joint_row_count_plan`, `physical_joint_distribution_plan` and operator witnesses only merge assignments with a common physical identity | `joint_graph.rs`, `joint_filters.rs`, `joint_distributions.rs`, `physical_realization/graph.rs`; `docs/protocol.md` physical DAG API; `schema/protocol.schema.json` graph contracts |
| **#2** Multi-parent, shared producer, JOIN/GROUP/HAVING/QUALIFY/set, selective projection | Exact filtered two-parent equijoins and singleton joined GROUP/HAVING compose through complete source bags in `joins.rs` and `grouping.rs`, including total CASE group keys; grouping/window/set subclasses remain owned by their operator-local implementations; unsupported variants stay residual | `tests/protocol_conformance_physical.rs` jointly checks positive and negative terminals, filter contradictions and DuckDB; other operator subclasses: `tests/physical_realization/{joint_operators,group_rank_set,nested_sets}.rs` |
| **#3** Materialization, write kind and complete negative/zero exclusion | Graph nodes preserve `write_kind` and do not mistake intermediate relations for writable physical sources. `joint_counts.rs` and `filters.rs` use closed-world row obligations; `write_states.rs` models ordered before/after physical state | `tests/physical_realization/{graph,negative_cardinality}.rs`, `tests/physical_write_states.rs`, `tests/protocol_conformance_physical.rs`; `docs/protocol.md` write-state contract |
| **#4** Cycles, inversions, contradictions, unknown and partial producers | `PhysicalProofGap`, `WitnessDirection` and `PhysicalWriteStatePlan` distinguish residual from demonstrable impossibility. Incomplete schema, ambiguous writers, unproved cross-row correlations and non-invertible expressions never yield arbitrary feasible constructions | `tests/physical_realization/{graph,joint_graph,joint_distributions,negative_cardinality}.rs`, `tests/physical_write_states.rs`; typed `gap` in active JSON schema |
| **#5** Reproducible sql-tdg-shaped DAGs, joint terminals and DuckDB oracle | The protocol owns seven raw scripts, eleven dbt models and four seeds. Positive filtered INNER JOIN and joined GROUP/HAVING goals now have exact jointly satisfiable source assignments, checked independently against actual DuckDB producer DAGs; other unproved fixtures retain typed residual evidence | `tests/protocol_conformance_{physical,dialect_laws,dbt_e2e}.rs`, `make dbt-e2e`, `tests/fixtures/conformance/v1/suite.json` |
| **#6** Unit/dialect/differential, API/JSON/schema/docs/adapters | Rust API has typed physical outcomes and deterministic ordering. The active graph JSON schema and `docs/protocol.md` expose canonical nodes, plans and typed source-state effects; dbt compiled manifest/catalog and direct SQL share semantics where their evidence is equivalent | `tests/{emission_contract,library_api,coverage_manifest,physical_write_adapter_parity,protocol_conformance_dialect_laws,protocol_conformance_dbt_e2e}.rs`, `schema/protocol.schema.json`, `docs/{adapters,protocol,code-map}.md` |

## Audit of feasible versus residual versus impossible

* **Identity/multiplicity:** repeated aliases and shared materializations keep
  one physical producer identity; join-key histograms and grouped/ranked/set
  witnesses may be lifted only when complete upstream assignments are
  constructively proved. Conflicting joint demands are `Impossible` only
  when contradictory exact facts are established.
* **Output constraints:** transparent renames and typed reversible projections
  can be inverted with source schema, NULL, type, frequency and count checks.
  Non-injective expressions or missing evidence are `Residual`, not a guessed
  histogram. Outcome domains and bounds remain authoritative, independent of
  whether source construction is possible.
* **Negative evidence:** zero output from truly empty source bags is distinct
  from nonempty closed-world bags whose every row satisfies `IS NOT TRUE`.
  Shared sources, alternative join branches, NULLs and duplicates require a
  single complete assignment; one rejected sample is not a whole-relation proof.
* **Writes:** verified INSERT/UPDATE/DELETE/MERGE and lifecycle cases carry
  ordered prior/after snapshots, source/target aliases, constraints and MERGE
  precedence. Unknown prior states, incomplete actions, unsupported DDL/session
  laws and unenforced constraints stay residual; contradictory complete states
  may be impossible.
* **Fail closed:** cycles, ambiguous/partial producers, unknown cardinalities,
  opaque operators, unproved correlation or expression inversion must not
  receive a feasible result. `PhysicalProofGap` and
  `PhysicalWriteStatePlan::Residual` retain typed diagnostic evidence.

Representative regressions exercise feasible, residual and impossible
directions, SQL NULL/UNKNOWN, duplicate bags, shared join sources, full
before/after state snapshots, canonical dialect evidence and raw SQL/dbt
adapter parity. `tests/task_68_signoff.rs` guards completed backlog ACs,
actual fixture classifications, reference paths, active schema and the
independent release boundary.

## Constructive positive closure and sound residual boundaries

`advanced_boundary.sql` proves a positive one-row
`core_enriched` using **complete** raw-orders and raw-customers
bags. A typed equality on a non-join source column holds for every
constructed physical row (integer `amount = 100`, Boolean
`active = TRUE`). A type-attested integer equijoin controls matching
keys and cardinalities; neither materialized stage is independently
writable.

`advanced_pipeline.sql` further proves exactly one grouped output
`mart_customer_summary`, with a total scalar CASE group key and
downstream `WHERE` known identical to the upstream complete source
filter. `HAVING COUNT(*) >= 1` is evaluated for one joined row.
A single sufficient case controls both terminal output counts, and an
independent DuckDB pipeline with one row in each physical input
verifies `stage_orders`, `stage_customers`, `core_enriched` and
`mart_customer_summary`, including computed key `high`.
Existing multi-terminal zero outputs remain feasible: positive join
capability does not turn joint closed-world empty-source proofs into
spurious source-count conflicts.

Typed Boolean `WitnessTerm` evidence is represented in Rust and
the JSON schema; a dialect that parses `TRUE` differently from a
Boolean literal is not upgraded to equivalent physical construction.
Unreconciled join-key filtering, a conflicting downstream predicate,
multi-join or outer-join singleton grouping and other unsupported
shapes remain typed `Residual`. Protocol-owned SQL/dbt conformance
and the coverage manifest guard these distinctions.

## Execution gates

PR verification must run the repository's CI jobs **on this sign-off
commit**, not infer success from prior child PRs:

```sh
make fmt
make lint
make test
make doc
cargo clippy --all-targets --no-default-features -- -D warnings
cargo test --no-default-features
cargo doc --no-deps --no-default-features
cargo tree --no-default-features --edges normal
make dbt-e2e
```

`make test` contains the conformance SQL, NULL/duplicates, cross-dialect and
DuckDB physical cases. `make dbt-e2e` runs both dbt projects, including the
11-model canonical suite and catalog-based adapter parity. The CI
`no-default-features / dependencies` job additionally rejects accidental
`saphyr` inclusion. Rust and JSON witnesses now include typed Boolean terms and the
filtered JOIN/GROUP population subset. These protocol-side features
are candidates for the separately gated 3.0.0 release; TASK-91, not
this sign-off, controls acceptance and actual publication.

## Explicitly not certified by TASK-68

This sign-off asserts sound complete physical proofs for every
*documented and constructively certified subset*, while unproved
general variants still fail closed. It does not
assert arbitrary positive multi-operator joins/groups/windows/sets, arbitrary
correlated EXISTS, non-bijective scalar inversion, unconstrained complete
distributions, all dialect-specific execution semantics, arbitrary session
behavior, or every DML/DDL sequence is constructible. Some concrete DuckDB examples still succeed even when the *general*
construction remains residual; the global release matrix must not
be upgraded on the strength of those examples.

`docs/coverage-manifest.json` deliberately keeps the global 13-dialect /
feature variant matrix at `unverified` and `not_end_to_end_proven` until
TASK-70..90 and TASK-88/89 prove their separate obligations. Consumer
sql-tdg TASK-31/35/36/43 still owns protocol-SHA pinning, generator-produced
rows, randomized rejection alternatives and whole-project generation oracles.

**TASK-91 and Release Please PR #79 remain blocked.** No release approval
or generator sign-off is implied by closing this parent task.
