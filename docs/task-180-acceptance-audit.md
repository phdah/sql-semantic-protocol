# TASK-180: independent dialect gate acceptance audit

Status: **In Progress, release blocked**. This is a bounded implementation PR, not TASK-88 or protocol 3.0.0 signoff.

## Frozen task-owned scope

- 163 immutable TASK-177/178 SQL cases by 13 exposed dialects = 2,119 cells. The original C/D/U/R observations are 2,014/23/55/27. See `docs/task-180-acceptance-freeze.json`.
- Every cell has a stable SQL/cell identity, source SQL, named owner, original canonical classification, separate independently reviewed bounded semantic disposition, positive/negative fixture IDs and explicit unattested vendor-native state.
- Source and terminal expectations are pinned to TASK-192..196 independent typed/domain/bag reviews and their full DuckDB oracles. TASK-197 owns direct SQL/dbt/ODCS fact parity, TASK-179 vendor/session limitations and TASK-191 original parser/Unsupported/native exceptions.
- `tests/task_180_independent_dialect_gate.rs` verifies the 2,119-row closed inventory against the original upstream matrix and independent adapter ledger, rejects silent release upgrades, and replays original S001..S003 complete source/terminal and TRUE/FALSE/UNKNOWN/NOT TRUE DuckDB row bags with 13-parser typed output checks.
- `docs/coverage-manifest.json` explicitly records the release blockers. No external vendor runtime has been executed or attested by this work.

## Acceptance criteria

| AC | Current disposition | Required to close |
| --- | --- | --- |
| #1 | Task-owned v1 freeze created, pending independent review | Review and approve full typed/domains/physical-cell matrix, with mandatory/conditional distinctions |
| #2 | No new semantic law asserted; upstream family evidence is indexed and checked | Resolve any unimplemented mandatory original cells, verify strongest sound domains and full physical proofs rather than parser-only status |
| #3 | Independent original DuckDB TRUE/FALSE/UNKNOWN and complete terminal bags replayed for initial three original cases | Independent full source/terminal and negative/impossible certification for all executable mandatory cells and approved native boundaries |
| #4 | 13-dialect and cross-adapter review ownership frozen; native/vendor absence explicit | Reconcile every reviewed dialect law, SQL/dbt/ODCS fact equivalence, and named unapproved vendor/session dispositions |
| #5 | CI gate added to Rust integration test suite; not yet signed off | Verify exact final branch head with fmt, strict Clippy, all tests/doc/no-default features, dbt E2E, active schema/goldens, pinned sql-tdg compatibility; record independent review and evidence |

## Current non-waivable blockers

- TASK-191 explicitly retains 116 original unsupported analyzer cells and 23 original native outcomes requiring complete classification/resolution. Review dispositions are **not** an implementation or native vendor attestation.
- The original TASK-178 subset has 55 observed Unsupported, 27 parser rejections and 23 canonical drift cells. These must remain explicit until independently adjudicated, never converted into blanket Exact claims.
- All external native vendor execution claims remain unattested. Native DuckDB controls cannot substitute for BigQuery, Snowflake, MSSQL, etc.
- No parent TASK-88 signoff, TASK-91 release approval, or task TASK-180 Done status is claimed.

Independent Matt skill two-axis review, exact-head CI and consumer evidence are still required before changing the checklist or release flags.
