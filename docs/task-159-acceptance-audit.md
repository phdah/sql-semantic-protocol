# TASK-159 independent acceptance audit

**Scope:** TASK-83 child 3/4, existing PR #180. The parent TASK-83 remains open for TASK-160 and independent original-AC review.

## Immutable acceptance lineage

- Preimplementation fixture freeze: `docs/task-159-acceptance-freeze-v1.json` (exact copy of originally committed v1).
- Archived v2 fixture freeze: `docs/task-159-acceptance-freeze-v2.json`. V2 adds typed source/post domains, full programs, native DuckDB expectations, and mandatory negative contradictions, retaining all original seven mandatory cases. The eighth mandatory cell is RENAME collision.
- Active v3 fixture freeze: `docs/task-159-acceptance-freeze.json`. V3 changes only the conditional standalone `RENAME TABLE t TO u` parser outcome to canonical unsupported for all 13 dialects. Its vendor execution law is not certified, so normalizing it as a portable physical RENAME would violate the frozen Residual disposition. All eight mandatory oracles and their outcomes remain unchanged.
- `tests/task_159_frozen_cases.rs` interprets every mandatory v2 case as a public SQL program and compares each before/after complete bag against native DuckDB; it also asserts typed domain frequencies, minimum/maximum, NULL membership, inclusive bounds, affected ordinals, named relations, and truth partitions. Impossible cases prove the native statement rejects without altering existing states.
- The freeze is test data, not a source of execution law. No dynamic acceptance snapshots are generated from the analyzer.

## AC mappings and evidence

| Child criterion | Evidence | Required result |
| --- | --- | --- |
| #1 Pre-frozen matrix | v1 history, v2 active freeze, manifest link and source/target expectation oracles | Every mandatory case independently checked; conditional cases retain own owner/disposition |
| #2 Canonical lifecycle | `src/protocol/writes.rs`, `src/analysis/ddl_lifecycle.rs`, `src/physical_realization/write_states/{lifecycle.rs,../write_states.rs}`, `src/bundle/state.rs` | DROP absent, TRUNCATE empty typed table, RENAME exactly preserved bag only with proven destination absence; IF EXISTS no-op only with known absence |
| #3 Physical/native oracle | `tests/task_159_frozen_cases.rs`, `tests/task_159_lifecycle.rs`, `tests/task_159_lifecycle_defaults.rs` | Full DuckDB source/intermediate/terminal rows and schema, duplicate occurrences and NULL, Impossible collision and nonexistent DROP |
| #4 Dialects and adapters | `tests/task_159_dialect_matrix.rs`, `tests/task_159_adapter_parity.rs` | Every shared AST or explicit parser boundary in all 13 dialects; equivalent direct/dbt catalog/ODCS prestate evidence; vendor/session variants remain Residual |
| #5 Final CI/review and parent | GitHub Actions PR CI and standards/spec review at fixed final head | All nine CI jobs green, every child mandatory cell covered, signoff in child only; parent TASK-83 AC remain open |

## Conservative execution boundaries

- Parser acceptance does not imply engine execution or transaction portability.
- Missing caller snapshots are not evidence that a relation does not exist. DROP IF EXISTS after an already witnessed DROP is an exact no-op, but an isolated DROP IF EXISTS is Residual.
- RENAME is Exact only if a prior full DROP of the destination proves the name is free. Collision with an existing target is Impossible; absent destination evidence remains Residual.
- Normalized scalar defaults are migrated with the physical table identity; DROP clears the physical constraint/default evidence. Re-CREATE after DROP is Residual pending generation-aware schema declarations owned by TASK-160. This prevents a stale ALTER schema from producing a false Exact result.
- DROP TABLE and TRUNCATE are never approximated by DELETE WHERE, and RENAME is never approximated by copying rows. Complete physical bags preserve duplicate and SQL NULL occurrences, including before/after typed domains.
- CLONE/LIKE, TEMP/TRANSIENT object lifetimes, vendor DROP VIEW and materialized storage, conditional CREATE, CASCADE, restart identity, and DDL transactions are conditionally non-exact. Owner TASK-159; no vendor portability claim can be released without separately verified engine/version evidence.
- dbt's compiled SELECT model is not an arbitrary DDL script. dbt catalog/manifest and ODCS provide equivalent typed metadata to independent SQL lifecycle operations but do not independently execute them.

## Review and signoff

Review the actual final PR head against `main` using Matt's code-review standards and spec axes. A passing CI is necessary but not sufficient. Sign off only when all frozen mandatory cells, parser/adapter contract, independent review and final-head CI are true. Cross-object graph closure and full CREATE/ALTER/SELECT/DROP composition remain TASK-160; no TASK-83 parent checkbox is auto-signed.


## Independent semantics review correction

The initial lifecycle analyzer incorrectly advertised the *old* table as the
producer after RENAME and the destroyed table as a producer after DROP. The
public graph seam now makes RENAME produce only its new relation name, DROP
produce no datasets (including anonymous), and TRUNCATE retain its table
identity. `tests/task_159_lifecycle_graph.rs` verifies the public graph and
wire output and protects downstream consumers from phantom sources.

## Two-axis self-audit and CI checkpoint (2026-11-02)

**Standards axis:** the implementation passes cargo fmt, strict Rust Clippy, full tests, no-default-feature checks, docs and dbt E2E (nine green jobs at https://github.com/phdah/sql-semantic-protocol/actions/runs/41488484532). Public SQL ASTs remain at analysis boundary, canonical lifecycle types at protocol boundary, and physical snapshots remain conditional. Consumer-facing wire-kind compatibility now has a Conventional Commit breaking-change footer for the planned 3.0.0 release. The repeated constraint inventory predicate in ALTER and lifecycle was extracted into one shared guard during the independent standards review.

**Spec axis:** eight mandatory v2 executable fixtures (six Exact, two Impossible), complete before/after DuckDB row/schema snapshots, NULL and duplicate counts, typed domain frequencies and bounds, 234 pinned 13-dialect parser classifications and SQL/dbt/ODCS parity are verified. A separate inspection found and fixed phantom DROP and old-name RENAME graph outputs plus default lineage and stale schema recreation. Conditional vendor/session/clone laws remain explicit Residual and TASK-83 remains open.

This self-audit is **not represented as an independent subagent/reviewer signoff**. A separate fixed-point Matt standards/spec review remains the final TASK-159 criterion before Done. The reviewed implementation CI head is `c59113784a5968b73845af238c7bed1f2b99433c`; metadata-only updates should also pass CI before merge.

## Independent review defect and remediation (2026-11-02)

**Spec axis finding, corrected:** Standalone `RENAME TABLE t TO u` was classified `lifecycle:rename` despite its conditional Residual status in v2. The physical planner could consequently treat a vendor-specific statement as Exact based solely on a prior DROP of the destination. The analyzer now emits `unsupported_vendor_rename_table`, and the physical plan fails closed. This is tested with complete physical evidence that otherwise enabled Exact and the pinned 13-dialect v3 parser matrix. ALTER TABLE RENAME TO retains bounded portable semantics. The archived v2 file preserves the prior acceptance evidence. Independent Standards and Spec review is complete; the corrected implementation head passed all nine CI jobs on 9356f69c01a180ef4a409aa258816cc11ee03d5d.

## Final independent two-axis review and child signoff (2026-11-02)

**Standards: PASS (0 outstanding).** Checked `AGENTS.md`, `CONTRIBUTING.md` and the Fowler smell baseline against the PR diff. Shared physical constraint checks now have one owner; no outstanding documented standard breaches. The public parser-independent action, updated schema, graph output and explicit nonportable boundaries remain consistent.

**Spec: PASS (0 outstanding for bounded child).** The v3 18-case inventory includes 8 independently executed mandatory DuckDB oracles (6 Exact, 2 Impossible); 234 pinned parser cells and applicable direct SQL/dbt/ODCS parity. The previously incorrect standalone vendor RENAME Exact claim is now Residual and regression-tested. No TASK-83 parent criterion is signed off by this child review.

**Implementation-head CI:** https://github.com/phdah/sql-semantic-protocol/actions/runs/41492788799 at `9356f69c01a180ef4a409aa258816cc11ee03d5d`, all nine jobs successful (fmt, strict Clippy, tests, docs, no-default checks, dbt E2E). TASK-159 AC #1 through #5 signed off. The metadata-only completion commit must also pass the PR integration CI before merge.
