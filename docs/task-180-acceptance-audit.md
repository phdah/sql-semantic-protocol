# TASK-180: independent dialect gate acceptance audit

Status: **In Progress, release blocked**. This is a bounded implementation PR, not TASK-88 or protocol 3.0.0 signoff.

## Frozen task-owned scope

- 163 immutable TASK-177/178 SQL cases by 13 exposed dialects = 2,119 cells. The original C/D/U/R observations are 2,014/23/55/27. See `docs/task-180-acceptance-freeze.json`.
- Every cell has a stable SQL/cell identity, source SQL, named owner, original canonical classification, separate independently reviewed bounded semantic disposition, positive/negative fixture IDs and explicit unattested vendor-native state.
- Source and terminal expectations are pinned to TASK-192..196 independent typed/domain/bag reviews and their full DuckDB oracles. TASK-197 owns direct SQL/dbt/ODCS fact parity, TASK-179 vendor/session limitations and TASK-191 original parser/Unsupported/native exceptions.
- `tests/task_180_independent_dialect_gate.rs` verifies the 2,119-row closed inventory against the original upstream matrix and independent adapter ledger, rejects silent release upgrades, and replays all 25 original TASK-177 predicate samples across complete DuckDB source/terminal, TRUE/FALSE/UNKNOWN/NOT TRUE row bags and 13-parser typed-domain checks. `tests/task_180_live_canonical.rs` separately executes each of the 2,119 original parser/canonical cells and verifies the reviewed 22-cell normalization overlay.
- `docs/coverage-manifest.json` explicitly records the release blockers. No external vendor runtime has been executed or attested by this work.

## Acceptance criteria

| AC | Current disposition | Required to close |
| --- | --- | --- |
| #1 | Task-owned v1 freeze created, pending independent review | Review and approve full typed/domains/physical-cell matrix, with mandatory/conditional distinctions |
| #2 | No new semantic law asserted; upstream family evidence is indexed and checked | Resolve any unimplemented mandatory original cells, verify strongest sound domains and full physical proofs rather than parser-only status |
| #3 | Independent original DuckDB TRUE/FALSE/UNKNOWN and complete terminal bags replayed for all 25 original predicate cases | Independent full source/terminal and negative/impossible certification for all executable mandatory cells and approved native boundaries |
| #4 | 13-dialect and cross-adapter review ownership frozen; native/vendor absence explicit | Reconcile every reviewed dialect law, SQL/dbt/ODCS fact equivalence, and named unapproved vendor/session dispositions |
| #5 | CI gate added to Rust integration test suite; not yet signed off | Verify exact final branch head with fmt, strict Clippy, all tests/doc/no-default features, dbt E2E, active schema/goldens, pinned sql-tdg compatibility; record independent review and evidence |

## Current non-waivable blockers

- TASK-191 explicitly retains 116 original unsupported analyzer cells and 23 original native outcomes requiring complete classification/resolution. Review dispositions are **not** an implementation or native vendor attestation.
- The original TASK-178 subset has 55 observed Unsupported, 27 parser rejections and 23 historical canonical drift cells. Twenty-two drift cells have a reviewed normalization overlay; one MSSQL LATERAL drift remains. See `docs/task-180-exception-and-gap-ledger.json` for the exact 13 affected SQL variants and their 105 exception cells. These must remain explicit until independently adjudicated, never converted into blanket Exact claims. The 55 original Unsupported cells are five write/DDL shapes (REPLACE INTO, INSERT OVERWRITE, IN-subquery UPDATE, IN-subquery DELETE and ALTER SET DEFAULT). Source-typed evidence can be stronger than raw SQL observations; do not infer permanent unimplemented status from a historical raw-SQL class alone.
- All external native vendor execution claims remain unattested. Native DuckDB controls cannot substitute for BigQuery, Snowflake, MSSQL, etc.
- No parent TASK-88 signoff, TASK-91 release approval, or task TASK-180 Done status is claimed.

Independent Matt skill two-axis review, exact-head CI and consumer evidence are still required before changing the checklist or release flags.


## Verified progress on this PR

- [Run 44480024335](https://github.com/phdah/sql-semantic-protocol/actions/runs/44480024335) passed all **9/9** jobs at `d8f7727`: native 25-case predicate source, terminal and three-valued truth replay, fmt, Clippy, tests with and without default features, docs and dbt E2E.
- The updated current-branch `tests/task_180_live_canonical.rs` independently executes all 2,119 current public SQL/dialect cases and compares them with the reviewed parser/canonical overlay. The exception ledger test requires all 105 originally non-C cells to retain an explicit owner/disposition and distinguishes 22 normalized drift cases from one unapproved drift.
- **Do not mark AC #1..#5 Done solely from those passing tests**: source-backed typed/physical evidence, independently approved conditional vendor boundaries and exact-head pinned consumer signoff remain independent requirements.

### Concrete unresolved semantic families

| Original fixture | Unresolved gate |
| --- | --- |
| S199 REPLACE INTO | 3 parser-accepted original Unsupported, 10 parser rejections; conflict-policy native law is not certified |
| S200 INSERT OVERWRITE | 13 original Unsupported; partition/whole-table replacement scope is not safely proved |
| S203 UPDATE with IN subquery | 13 original raw-SQL Unsupported; TASK-196 source-typed bounded path exists, requires independent final typed/native review |
| S206 DELETE with IN subquery | 13 original raw-SQL Unsupported; same source-typed proof boundary |
| S220 ALTER SET DEFAULT | 13 original Unsupported; typed default DDL/schema effect unproven in the raw original |
| S152 LATERAL (MSSQL) | 1 outstanding canonical drift, parser/native dialect-specific boundary |
| S084 CORRESPONDING | 13 original parser rejections, not equivalent to independently proven BY NAME |
| S120, S179, S189 | Named Snowflake WINDOW, structural CAST and PostgreSQL generated-column parser boundaries |

## Exhaustive original native-evidence replay

TASK-180's original SQL suite now covers all **163** source variants with either an executed DuckDB original / explicitly reviewed equivalent or a declared non-executable/conditional boundary:

| Family | Original SQLs | Independently exercised evidence |
| --- | ---: | --- |
| TASK-192 scalar predicates / types | 43 | 25 baseline 3VL bags, 11 additional source-instance 3VL partitions, 4 string-pattern cases, 3 overflow / ordering / STRUCT controls |
| TASK-193 joins / scopes | 24 | 24 complete join native bags; original SEMI/ANTI native parsing explicitly distinguished from their executable DuckDB equivalents |
| TASK-194 sets / alignment | 22 | 20 native source/terminal and rejected-candidate bags; 2 named parser / arity boundary cases |
| TASK-195 grouping / windows / pagination | 36 | 25 integer and 3 float deterministic complete bags; 8 native conditional / stochastic / parser boundaries |
| TASK-196 DML / DDL / metadata | 38 | 16 complete DML before/after/RETURNING states; 18 DDL constraint/lifecycle poststates; 2 native syntax rejections and 2 metadata authority boundaries |

**149 native evaluated controls plus 14 explicit unexecutable/conditional controls** cover every case, without asserting unexecuted vendor execution. The 13-dialect public/canonical matrix is separately executed across all 2,119 frozen cells by `tests/task_180_live_canonical.rs`.

Executable files: `tests/task_180_independent_dialect_gate.rs`, `tests/task_180_scalar_native.rs`, `tests/task_180_original_write_states.rs`, and `tests/task_180_live_canonical.rs`.

`docs/task-180-final-signoff-evidence.json` must contain an independently reviewed record for every one of the 2,119 cells, all 481 vendor/session dispositions, final-head source and pinned-consumer CI and schema/golden checks before TASK-180 may become Done. The test gate rejects a state transition lacking that evidence. Native fixture reproducibility alone is not task-wide semantic approval.

## Vendor/session live classification and acceptance safety

`tests/task_180_live_canonical.rs` also executes all **37 TASK-179 original vendor/session SQL laws × 13 project dialects = 481 cells**. The frozen and runtime-classified matrix has 442 parsed/analyzed, 35 parsed/explicitly unsupported, and 4 parser-rejected cells. Every frozen vendor-native status remains `unattested` and original release disposition remains `unapproved_vendor_law`.

The independent completion gate now requires **2,119 distinct original cell approvals and 481 distinct reviewed vendor-law dispositions** with original identities, reviewer, physical and typed evidence, specific source/adapter conditions, and actual native engine evidence whenever claimed. It also requires exact-head protocol CI, pinned sql-tdg CI, dbt E2E and schema/golden review. All original AC #1..#5 remain unchecked pending that substantive evidence; green reproducibility tests cannot auto-approve a mandatory semantic or vendor law.
