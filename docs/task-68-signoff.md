# TASK-100: TASK-68 physical-source composition sign-off

This is the bounded **protocol-side** sign-off of TASK-68, not approval to
publish 3.0.0 or certify sql-tdg's generator. The authoritative physical
construction result is a typed `Feasible`, `Impossible` or `Residual`
decision. A DuckDB example that happens to execute does **not** upgrade a
residual into a general constructive proof.

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

## Parent acceptance-criteria traceability

| TASK-68 AC | Implementation and safety argument | Checks and public contract |
| --- | --- | --- |
| **#1** Physical identity, truth, lineage, multiplicity and witness composition | `PhysicalPlanRef` deduplicates controlled leaves; `physical_joint_source_plan`, `physical_joint_row_count_plan`, `physical_joint_distribution_plan` and operator witnesses only merge assignments with a common physical identity | `joint_graph.rs`, `joint_filters.rs`, `joint_distributions.rs`, `physical_realization/graph.rs`; `docs/protocol.md` physical DAG API; `schema/protocol.schema.json` graph contracts |
| **#2** Multi-parent, shared producer, JOIN/GROUP/HAVING/QUALIFY/set, selective projection | Operator-local proofs in `joins.rs`, `composition.rs`, `grouping.rs`, `ranking.rs`, `sets.rs`; reversible projection and value distribution in `distributions.rs`; uncertified mixed paths stay residual | `joins.rs`, `joint_operators.rs`, `group_rank_set.rs`, `nested_sets.rs`, `joint_distributions.rs` under `tests/physical_realization/`; cross-dialect conformance |
| **#3** Materialization, write kind and complete negative/zero exclusion | Graph nodes preserve `write_kind` and do not mistake intermediate relations for writable physical sources. `joint_counts.rs` and `filters.rs` use closed-world row obligations; `write_states.rs` models ordered before/after physical state | `tests/physical_realization/{graph,negative_cardinality}.rs`, `tests/physical_write_states.rs`, `tests/protocol_conformance_physical.rs`; `docs/protocol.md` write-state contract |
| **#4** Cycles, inversions, contradictions, unknown and partial producers | `PhysicalProofGap`, `WitnessDirection` and `PhysicalWriteStatePlan` distinguish residual from demonstrable impossibility. Incomplete schema, ambiguous writers, unproved cross-row correlations and non-invertible expressions never yield arbitrary feasible constructions | `tests/physical_realization/{graph,joint_graph,joint_distributions,negative_cardinality}.rs`, `tests/physical_write_states.rs`; typed `gap` in active JSON schema |
| **#5** Reproducible sql-tdg-shaped DAGs, joint terminals and DuckDB oracle | The protocol **owns** the versioned seven-raw-script, eleven-dbt-model fixture suite and four source seeds. `suite.json` states exact or residual outcomes; consumer SHA is only historical provenance. Physical source plans and DuckDB complete result bags are checked independently | `tests/protocol_conformance_{physical,dialect_laws,dbt_e2e}.rs`, `make dbt-e2e`, `docs/conformance-suite.md` |
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

Representative regressions explicitly exercise feasible, residual and impossible
directions, SQL NULL/UNKNOWN, duplicate bags, alternative join routes, full
state snapshots, cross-dialect canonical identities and raw SQL/dbt adapter
parity. `tests/task_68_signoff.rs` guards the task queue, reference paths and
release boundary so the closure cannot drift silently.

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
`saphyr` inclusion. The full active JSON contract is covered by Rust emission
and schema tests; **no protocol version bump** follows from this audit-only
closure.

## Explicitly not certified by TASK-68

The closure asserts that each *documented, actually constructible subset*
has a sound physical proof, and all unproved variants fail closed. It does not
assert arbitrary positive multi-operator joins/groups/windows/sets, arbitrary
correlated EXISTS, non-bijective scalar inversion, unconstrained complete
distributions, all dialect-specific execution semantics, arbitrary session
behavior, or every DML/DDL sequence is constructible. Some concrete DuckDB
examples succeed despite general plans being residual. This is not a defect
that may be hidden by upgrading the corresponding coverage cell.

`docs/coverage-manifest.json` deliberately keeps the global 13-dialect /
feature variant matrix at `unverified` and `not_end_to_end_proven` until
TASK-70..90 and TASK-88/89 prove their separate obligations. Consumer
sql-tdg TASK-31/35/36/43 still owns protocol-SHA pinning, generator-produced
rows, randomized rejection alternatives and whole-project generation oracles.

**TASK-91 and Release Please PR #79 remain blocked.** No release approval
or generator sign-off is implied by closing this parent task.
