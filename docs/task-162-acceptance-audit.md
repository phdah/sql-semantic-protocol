# TASK-162 bounded acceptance audit

**Status:** Child TASK-162 Done on 2026-11-05 with verified technical acceptance and disclosed connected-author-account review exception. TASK-84 remains open; PR #189 is the integration gate. No independent second-person review is claimed.

## Frozen contract, scope and evidence

- The v1 finite preimplementation inventory was committed before transaction semantics in `docs/task-162-acceptance-freeze-v1.json`. The immutable v2 adds observed parser classifications, archived separately. Active v3 adds independent native evidence boundaries for un-attested and explicitly enforced PRIMARY KEY metadata and for separate-step ALTER defaults. All versions remain available for audit.
- Ten named frozen cases: eight required Exact, Impossible or explicit missing-evidence Residual classifications, and two conditional engine/mixed-script Residual boundaries. Parent TASK-84 acceptance #2 through #6 and child #1 through #5 have explicit freeze mappings.
- All 13 exposed dialect identities have seven pinned transaction-control parser classifications (91 cells). BigQuery and Snowflake BEGIN parser boundaries are preserved as parser boundaries, not promoted to execution. Only DuckDB's pinned native engine is used for exact transaction claims.
- `tests/task_162_transactions.rs` and `tests/task_162_sql_frontend.rs` independently compare complete DuckDB physical catalogs, row occurrence identity, duplicate and NULL bags, schema before/after, affected counts, read TRUE/NOT TRUE partitions, rollback of CREATE/UPDATE/DELETE, and invalid transaction controls.
- `tests/task_162_adapter_parity.rs` checks representable equivalent SQL/dbt/ODCS authoritative schema evidence yields identical typed transaction outcomes. Neither dbt nor ODCS is treated as transaction executor.
- `tests/task_162_dialect_matrix.rs` and `tests/task_162_acceptance_inventory.rs` gate dialect coverage, versioned matrix identity, original child AC and parent mapping. `docs/coverage-manifest.json` registers the evidence paths.

## Child acceptance criteria

| AC | Verified result |
| --- | --- |
| #1 Bounded freeze | Immutable pre-semantics v1, explicit observed v2/v3 deltas, 10 stable cases, source/terminal expected bags and typed value domains, 91 dialect cells, positive and NOT TRUE contrast, adapter and conditional owners; no unchecked mandatory cell |
| #2 Canonical sequential transaction proof | Real SQL controls normalized once at the AST boundary into typed actions; canonical TASK-161 graph proves each DDL/DML/read. BEGIN snapshots complete catalog and enforced constraints; COMMIT keeps exact bag; ROLLBACK restores exact rows, schemas and evidence. Invalid controls Impossible; unsupported context Residual |
| #3 Native positive and negative oracle | Full DuckDB source, intermediate and target snapshots including NULL/duplicate occurrences, exact domains, mutation counts, read survivor/reject ordinals, invalid/nested and enforced duplicate-key cases. Un-attested SQL PK enforcement is explicitly Residual; attested enforced PK Exact |
| #4 All dialects and representable adapter parity | 13x7 pinned parser matrix, native DuckDB engine boundary and savepoint negative executable oracle, direct SQL/dbt/ODCS schema-equivalence parity; parser success never implies engine support |
| #5 Implementation-head CI and reviewed acceptance | Nine of nine CI jobs successful at `3d85a7806981118473cb92fb0935a3e56cec6c2d`, including fmt, strict Clippy all-features/no-default, Rust tests, docs, dbt E2E and dependency checks. No active root/schema/protocol version change. Two-axis connected-author-account review recorded below; separate-person review process exception transparently disclosed |

## Conditional release boundaries, no false exactness

- **SAVEPOINT / ROLLBACK TO / RELEASE:** sqlparser accepts some vendor syntax, but the pinned `duckdb ~1.10506.0` native test rejects these operations. This is explicitly typed `Residual`, owned by TASK-162. No fabricated savepoint or engine-law proof is emitted.
- **Separate-step ALTER defaults:** A standalone canonical ALTER may be supported by TASK-158, but carrying updated default/constraint evidence to a separately analyzed later SQL step is not yet certified. The transaction adapter returns `Residual` instead of silently losing a default. Full mixed-script metadata propagation remains TASK-164 scope.
- **Un-attested primary keys:** SQL DDL declares the key but canonical enforcement provenance is `Unknown`. Without attestation it remains `Residual`; caller supplied `ConstraintEvidence::Enforced` allows the canonical constraint validator to prove valid rows or return `Impossible` for duplicates.
- **Unsupported sessions, triggers, implicit vendor rules, concurrent isolation, uncommitted final transaction:** Unproved assumptions never become exact. Parent TASK-84 retains separate cross-feature closure.

## Implementation verification and review provenance

- **Implementation SHA:** `3d85a7806981118473cb92fb0935a3e56cec6c2d`
- **CI (9/9 green):** https://github.com/phdah/sql-semantic-protocol/actions/runs/42109605476
- **Matt Pocock implement + two-axis code review:** https://github.com/phdah/sql-semantic-protocol/pull/189#pullrequestreview-5792140694. Standards PASS and Spec PASS for bounded TASK-162, with no outstanding code blocker at this reviewed implementation.
- **Reviewer provenance:** The review was submitted using the PR author's connected GitHub account, **not a separate independent human reviewer or different GitHub identity**. The owner requested completing TASK-162's acceptance criteria in chat on 2026-11-05; this child records an owner-directed connected-author-account review-process exception, following the project precedent. It is not represented as independent peer approval; repository merge policy can still require separate approval.
- **Compatibility:** Additive opt-in library physical proof API. Active emitted JSON root/schema, `PROTOCOL_VERSION` and protocol contract remain unchanged. `docs/protocol.md`, `docs/semantics.md` and `docs/code-map.md` document boundaries.

**Disposition:** All TASK-162 bounded technical AC #1..#5 are completed and signed off with the stated process exception. TASK-84 parent remains To Do and PR merge is a separate integration gate. TASK-163 remains blocked until PR #189 is merged into `main`.
