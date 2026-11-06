# TASK-173 acceptance and review audit (2026-11-06)

**Scope:** TASK-173 child 1/4 of TASK-87. The parent remains To Do and the v3 release remains gated. Bounded technical signoff covers the eight unchanged mandatory case identities from the pre-semantics v1 freeze, not all of TASK-87.

## Freeze and traceability

- Immutable pre-semantics inventory: [v1](task-173-acceptance-freeze-v1.json).
- [v2](task-173-acceptance-freeze.json) adds explicit 104 case/dialect and 24 case/adapter cells, without weakening a v1 fixture, original acceptance criterion, source bag or expected domain. This evidence expansion was recorded after the initial implementation; it is not represented as a second preimplementation freeze.
- [Verification](task-173-acceptance-verification.json) owns executed results; the `verified: false` snapshots in the frozen expectation inventory are the **preverification baseline**, not final test results.
- Fixed oracle: complete source `raw.orders = [1, 1, NULL, 0]`; positive terminal `[1, 1]`; FALSE source occurrence `0` at ordinal 3; UNKNOWN NULL at ordinal 2. Full source frequency `[(1,2), (NULL,1), (0,1)]`, output `[(1,2)]`, inclusive lower bound 1.

## Original child acceptance criteria

| AC | Evidence and disposition |
| --- | --- |
| #1 Frozen inventory | v1 predates code; v2 preserves scope and makes 104 parser and 24 adapter classifications inspectable. Each variant has a fixed source/terminal oracle and typed interval/NULL evidence; conditional vendor/constraint boundaries have owner and release disposition. |
| #2 Canonical semantics | `src/dbt.rs` retains authoritative compiled model SQL, dependency identities, manifest constraints/tests, catalog warehouse datatypes and explicitly lower-authority manifest fallback. `src/odcs.rs` now keeps valid `physicalType` with `odcs_unsupported_logical_type` when its logical hint is unrecognized. `SchemaSourceKind` and `ConstraintSourceKind` survive typed analysis, with enforcement `Unknown` unless independently attested. |
| #3 Native full-bag oracle | `tests/task_173_metadata_authority.rs` compares full DuckDB physical source, terminal, FALSE and UNKNOWN bags; canonical `physical_ordered_lifecycle_plan` proves complete before/after source and terminal row identities, exact histogram counts, source/terminal schemas, inclusive lower bound and rejected ordinals for direct DDL, dbt catalog, dbt manifest fallback and ODCS. Missing schemas and unrecognized standalone ODCS logical types do not receive fabricated Exact proofs. A duplicate source violating a dbt-declared unique test with unknown enforcement returns `Residual`. |
| #4 Dialect/adapter evidence | All 8 × 13 canonical portable filter parser cells are exercised; 24 adapter cells record matched schema facts or explicit not-applicable boundaries. ODCS and dbt are evidence, not claims of native non-DuckDB engine execution. |
| #5 Final CI and review | [Implementation-head CI](https://github.com/phdah/sql-semantic-protocol/actions/runs/42258873335) passed all nine jobs: fmt, strict Clippy, test/doc with and without default features, dependency validation and real dbt Core E2E. Existing active schema/golden tests remain green; public root protocol/schema were not changed. Review carried out on standards and spec axes as an owner-directed same-account pass, **not** an independent second human. |

## Case-level disposition

| Frozen case | Verified behavior |
| --- | --- |
| `87-C173-P-dbt-compiled` | Compiled SQL filter wins over conflicting raw_code, dependencies preserve canonical source identity, typed domain lower bound >= 1 |
| `87-C173-P-dbt-catalog` | Catalog INTEGER wins over manifest-declared TEXT, with warehouse provenance |
| `87-C173-P-dbt-manifest-fallback` | Complete manifest-declared INTEGER is used with declared-only provenance when catalog absent |
| `87-C173-P-dbt-constraints` | Source unique constraint and generic unique test preserve two distinct provenance records with unknown enforcement; physical duplicate remains Residual |
| `87-C173-P-odcs-physical` | Valid physical INTEGER survives unsupported logical hint with explicit diagnostic, retaining external-metadata provenance |
| `87-C173-P-direct-ddl` | SQL DDL INTEGER source kind retained; execution only independently attested in DuckDB |
| `87-C173-N-no-catalog-no-declaration` | Missing catalog and manifest datatype fail explicitly; no inferred warehouse type |
| `87-C173-N-odcs-no-types` | Missing physical/logical types emit unresolved and incomplete schema diagnostics; no typed source emitted |

## Conditional and external gates

- Runtime constraint enforcement from dbt data tests and ODCS `required` flags is **Residual/Unknown** without separate warehouse attestation. The native source-violation test proves that these declarations cannot be promoted to Exact enforcement.
- Vendor SQL dialect parsing is **not** native engine/session execution. Only DuckDB complete-bag outcomes are certified. Pin native vendor version/session and run full source/terminal/NOT TRUE oracles before any stronger claim. Owner: TASK-173; release decision remains TASK-91.
- The current sql-tdg `Cargo.toml` uses published protocol `2.0.3` with `default-features = false`, and the consumer has no ODCS adapter API path. This PR changes ODCS source parsing and test/docs only, not the public protocol type/wire contract. A consumer-pinned v3 candidate test is **not applicable to this bounded change**; the required cross-repo integration is independently owned by sql-tdg TASK-43 and protocol TASK-90/91. No claim of having run consumer CI is made.

## Two-axis review and provenance

**Standards:** PASS. Core parser/analysis/emission separation remains intact, no new dependencies, AST exposure, public protocol API, `unsafe`, lint suppressions, or nondeterministic serialization. Error/diagnostic handling remains fail closed; public API tests assert semantic outcome domains and not just parsing. No outstanding hard standards finding.

**Spec:** PASS on the eight frozen child cases. Manifest/catalog/ODCS/direct-DDL fact owners and missing-evidence boundaries are explicit. Canonical output/value-domain and complete physical source obligations are demonstrated; conditional external laws are classified, not overclaimed. Parent TASK-87 remains open for TASK-174..176.

**Review provenance:** Same connected author account performing a separate two-axis review under the owner's direct request for TASK-173 signoff. This is an owner-directed review-process exception, **not** a claim of independent second-person review. GitHub merge remains a separate integration gate.
