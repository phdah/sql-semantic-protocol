# SQL Semantic Protocol: canonical conformance suite

## Ownership and versioning

**The canonical suite belongs to `phdah/sql-semantic-protocol`.** Its
machine-readable entrypoint is
[`tests/fixtures/conformance/v1/suite.json`](../tests/fixtures/conformance/v1/suite.json).
The `schema_version: 1` identifies the suite manifest format, not the emitted
protocol JSON version. The protocol release or **protocol Git commit SHA** is
the reproducible source of truth for the fixtures, expected outcomes and API.
Consumers pin that protocol revision; the protocol never pins a consumer.

The initial SQL/dbt examples were adopted from `phdah/sql-tdg` at commit
`90ec0e12a2d5cd2c1294556336fc947e176c9db9` as *historical provenance
only*. This is not a maintained import, a CI dependency, or an authority over
expected semantics. New cases and corrected expectations are authored and
reviewed here, with ordinary protocol Git history.

| Suite asset | What is certified |
| --- | --- |
| `v1/advanced_pipeline.sql` | Multi-parent joined GROUP/HAVING, jointly empty and one-row positive constructive physical populations, joint terminal proofs and DuckDB oracles |
| `v1/advanced_boundary.sql` | Exact closed-world positive filtered equijoin population and empty-source negative boundary |
| `v1/set_operations.sql` | UNION ALL, UNION, INTERSECT, EXCEPT, DISTINCT, LIMIT, NULLs and duplicate multiplicity |
| `v1/raw_sql/window_ranked.sql` | Ranking/QUALIFY with excluded NULL/duplicate sources |
| `v1/raw_sql/insert_workflow.sql` | Ordered INSERT SELECT, exact typed before/after source-row state |
| `v1/raw_sql/unsupported_exists.sql` | Correlated EXISTS dependency, concrete inclusion and NULL nonmembership |
| `v1/raw_sql/range_sampling.sql` | Inclusive interval bounds, positive physical counts and negative exclusions |
| `v1/dbt_core_project/models/*.sql` | Eleven dbt models, canonical dbt graph and complete terminal value snapshots |
| `v1/dbt_core_project/seeds/*.csv` | Four materialized physical input bags, including duplicate output witnesses |
| `v1/dbt_core_project/models/sources.yml` | dbt source metadata, manifest and warehouse catalog adapters |

The separate `dbt_fixture_inventory` in the release coverage manifest tracks the
**downstream** sql-tdg generator's unverified acceptance work. Its `consumer_repo`
and `consumer_project_path` are not canonical fixture sources.

The versioned `suite.json` lists seven SQL cases by stable ID, family, SQL path,
dialect, expected proof classification and typed row-count goals, and 11 dbt models with concrete
expected row counts and SQL expressions for sorted terminal value snapshots.
The raw SQL row-count goals are executed directly against the protocol's typed
physical planner on every test run. The suite also enumerates seed paths,
source population counts and release gates.
Expected output expressions are **DuckDB oracle projections**, not instructions
for consumers to reparse SQL or reconstruct semantic proofs.

## Running and consuming the suite

From the protocol repository:

```sh
cargo test --test protocol_conformance_physical
cargo test --test protocol_conformance_dialect_laws
make dbt-e2e
```

`make dbt-e2e` seeds the complete source relations, runs the actual dbt
project, generates manifest/catalog and executes
`tests/protocol_conformance_dbt_e2e.rs`. The test compares each model to
the **machine-readable suite expectations**, including counts, value
snapshots, NULL rejection and UNION ALL multiplicities.

For **sql-tdg or any other consumer**:

1. Pin `phdah/sql-semantic-protocol` by the same Git SHA as the protocol library
   dependency, or a published version containing the suite.
2. Read `tests/fixtures/conformance/v1/suite.json` and follow its relative SQL,
   dbt model and seed paths; never treat a consumer-side copy as authoritative.
3. Analyze each input with the protocol library (direct SQL or dbt artifacts);
   consume its canonical outcome facts and typed `Feasible`, `Residual` or
   `Impossible` physical plans. Do **not** reparse SQL to infer missing facts.
4. Execute application-specific behavior against the listed physical-source
   populations and independent DuckDB outcomes. For generated data, verify
   actual presence, absence, counts and distributions per terminal.
5. Fail closed on residual/unsupported construction. Successful parsing,
   semantically correct lineage, or one executable example is **not** a
   universal generator feasibility certificate.

The suite is a **portable Git-versioned artifact**, not a consumer-code
dependency, Git submodule or promise of a stable HTTP download endpoint.
Changes to known cases, proof labels or expected outputs require review and
passing protocol CI. New cases should extend the suite without weakening
previously certified outcomes.

## Exactness and release boundaries

Only canonical `WitnessDirection::Feasible` or
`PhysicalWriteStatePlan::Feasible` cases have certified complete physical
source constructions. `Residual` requires the generator to fail closed;
`Impossible` means a proven contradiction.

Source-backed range counts, jointly empty multi-parent DAGs, source-state
NULL/duplicates and the checked INSERT SELECT transition are exact.
The in-scope `advanced_boundary.sql` filtered equijoin and
`advanced_pipeline.sql` singleton GROUP/HAVING have **complete positive
closed-world physical-source proofs**, including one common assignment for
their joined and grouped terminals. They are checked independently against
DuckDB. More general joined/grouped populations, ranked/window
distributions or correlated EXISTS can succeed on concrete DuckDB sources
while their constructive plans remain residual; do not upgrade those cases
from oracle execution alone.
Other SQL dialect variants and UPDATE/DELETE/MERGE/CTAS operator-state tests
live in focused protocol suites; generator execution of those workloads is
separately unverified.

TASK-100 audits TASK-68 proof closure; TASK-88/89 audit broader dialect and
operator oracles; TASK-91 gates protocol 3.0.0. Downstream sql-tdg
TASK-24..31/35/36 must independently pass randomized rejection, complete
generated source bags and end-to-end scripted DML/DDL. None of this suite's
protocol-side acceptance waives those release requirements.
