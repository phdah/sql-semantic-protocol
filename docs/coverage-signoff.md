# Maintainer-approved v3 scope and acceptance decisions

**Decision status: approved 2026-10-09, with the four addendums below.** This is approval of engineering *requirements and scope*, **not** release sign-off, not evidence of implemented generation, and not certification of any unchecked coverage cell.

The [coverage matrix](coverage.md), [executable manifest](coverage-manifest.json), [TASK-66](../.backlog/tasks/task-66%20-%20inventory-sql-dialect-feature-contract.md) and [TASK-91](../.backlog/tasks/task-91%20-%20gate-single-protocol-3-release-on-tdg-signoff.md) remain authoritative for test evidence and the release gate.

## 1. Per-terminal rejected rows with randomized failure predicates: approved

- Classify each generated physical source row against **every selected terminal outcome**. A row can be qualifying for terminal A and provably rejected for terminal B. Persist the vector and stable outcome identities as protocol-defined semantics, with independent SQL execution verification.
- **Randomly choose which eligible rejecting predicate or column to violate**, using the injected, reproducibly seeded generator RNG; do not hard-code the first predicate or always falsify the same column. Enumerate all *provably constructive* rejection alternatives expressed by the protocol, including nested AND/OR/NOT, joins, null-sensitive predicates, computed/aggregated/window predicates, and multi-layer composition where exact semantics are available.
- Seeded randomized selection must sample across the eligible choices. Tests with multiple seeds must demonstrate **every** feasible alternative can be exercised, including different columns, while fixed seeds reproduce identical generated tables, selections and classifications. Do not falsely require that a single row fail all columns or that a small random sample deterministically reaches each alternative.
- **Only select a predicate when its violation guarantees nonmembership in the intended final terminal**, after all Boolean logic and alternative lineage paths are evaluated. Example: failing one side of `A OR B` is *not* sufficient if B passes; instead the candidate must prove that the entire output predicate is FALSE or UNKNOWN and the row cannot contribute through another branch. For `NOT`, `EXISTS`, sets, joins, aggregates, windows, and mutations the proof may require multiple coordinated source rows, not a single-column mutation.
- Preserve requested matching/rejected counts, schema/uniqueness/FK/dbt tests, and per-terminal counts. Report a typed impossible or residual error naming terminal and rejected alternative when no exact witness is constructive; never reduce counts, silently drop a predicate, or present a merely plausible row as rejected.
- **The default `--rejected 10` must work in whole-project mode** for fully proven compatible workloads. Scenario partitioning and shared/disjoint source cases need complete metadata and independent SQL checks.

Owner: upstream protocol TASK-86 and TASK-68/70/85, sql-tdg TASK-35; final sql-tdg TASK-36.

## 2. Rebuild and expand the complete dbt/SQL E2E gate: approved

- **Rebuild/extend the committed dbt DuckDB end-to-end workflow**, rather than limiting the fixture to read-only models. Its `make all` must cover all **reviewed, protocol-supported** transformation and DML/DDL families, including INSERT, INSERT SELECT, UPDATE, DELETE, MERGE, UPSERT/conflicts, CTAS, CREATE VIEW, CREATE OR REPLACE, DROP/ALTER, transactions, ordered effects and post-state where applicable.
- dbt-native models, including incremental materializations and snapshots, should exercise their feasible DML/DDL behaviors; use a **companion DuckDB scripted state-transition harness** for SQL that dbt's model DAG cannot natively express. This harness is mandatory, invoked by the **same top-level `make all`** and CI gate; it is not an optional or independent substitute for the dbt E2E.
- Generate and load the protocol-driven initial physical sources/target state; run the full SQL program in DuckDB; assert **complete deterministic output rows, row multiplicities, source-to-terminal membership and before/after table contents**, including untouched rows, keys, NULLs, conflict paths, matched/unmatched MERGE paths and valid/idempotent versus non-idempotent effects.
- Include every inventoried in-scope group/window/set/CTE/join/subquery/predicate combination, DML and DDL variant that the protocol promises. For dialect-specific syntax that DuckDB cannot execute verbatim, separately parse/analyze it with that dialect and compare canonical semantics with an equivalent DuckDB-executable SQL fixture where an equivalence is genuinely proven. Do not claim vendor-runtime conformance from DuckDB alone.
- The complete audited feature/dialect manifest is the finite acceptance target; newly discovered supported transformations must extend the inventory and required tests. No claim of literal exhaustive `every SQL ever` for arbitrary external code is made.

Owner: sql-tdg TASK-31 and TASK-36, protocol TASK-80..84 and TASK-89.

## 3. Non-supported features remain future extensibility targets: approved

- Uninterpreted arbitrary UDFs, unseeded stochastic functions, environment-sensitive session behavior, unbounded/nonterminating recursion and opaque vendor operators are **not permanently forbidden**. They are **deferred pending sound semantic evidence** and explicit user-provided execution assumptions or supported metadata.
- Future contracts may accept declared function properties/return-domain expressions from structured input, introspection queries against a particular database, dbt macro/function metadata, ODCS or another verifiable adapter. This also applies to seeded stochastic behavior, recursion bounded by provable termination, and vendor session/collation/timezone laws. The canonical source-independent protocol must own typed validated semantics and provenance; adapter-specific syntax stays at the parsing/evidence boundary.
- Without such evidence, exact generation **fails closed** with an explicit unsupported/residual reason. A syntax being parseable is never enough. Do not guess a UDF's value domain or side effects from its name.
- Keep explicit documented deferrals and follow-up tasks; do not call a deferred capability 'supported', but also do not phrase scope as an irrevocable exclusion from later versions. Exact, safely bounded subsets are not automatically excluded.
- Approval of this deferral policy does not approve missing tests for currently *claimed* supported variants.

Owner: protocol TASK-66/78/79/87/88/90 and future scoped extension tasks, sql-tdg TASK-33/36.

## 4. All supported dialects must produce equivalent canonical protocols: approved

- Before final v3 sign-off, run **parser and analyzer tests on CI for all 13 supported dialect families**, covering every inventoried supported feature/variant which parses in that dialect, including dialect-specific SQL renderings. Do not rely on warehouse availability to test parsing.
- Where variants express the **same SQL meaning**, compare the normalized parser-independent protocol **including outcome domains, bounds/inclusivity, three-valued truth, lineage, cardinality, positive/negative requirements, write effects and residual status**. Ignore only source metadata identifying the dialect and other explicitly non-semantic presentation metadata. Do not weaken assertions to 'SQL parses' or 'JSON shape matches'.
- Where semantics actually differ by dialect or session settings, document conditional laws and explicit typed assumptions; do not require false equivalence. If a supported form cannot be proven equivalent, mark it residual and **block sign-off** for that claimed support.
- **DuckDB is the executable E2E oracle** for equivalent SQL transformations and generator results; execute generated physical data against its full transformation scripts. A DuckDB pass cannot by itself certify BigQuery/MySQL/PostgreSQL/Snowflake/etc. native engine behavior. This distinction must remain explicit in CI reports and the manifest.
- The tests run in CI even if no external database is provisioned. All 13 names are metadata inventory only, not a hard-coded runtime dialect whitelist.

Owner: protocol TASK-88/89 and sql-tdg TASK-33/36.

## Separate final release gate

**Release Please PR #79 stays unmerged.** Protocol 3.0.0 release sign-off is distinct from the four decisions above. TASK-66..90, cross-dialect semantic-equivalence tests, exact generator-ready physical-source positive/negative proofs, randomized rejecting-alternative coverage, the complete dbt `make all` + scripted DML/DDL E2E, pinned prepublication protocol candidate integration in sql-tdg TASK-43, and TASK-91 must all pass before the maintainer can approve publishing the consolidated release.

**Approval recorded for scope decisions 1–4; none of the implementation evidence, unfinished task acceptance criteria or final release authorization has been waived.**
