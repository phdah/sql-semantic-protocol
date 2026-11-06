# TASK-163 bounded acceptance audit

**Status:** TASK-163 Done for its eight frozen cases on 2026-11-06; bounded technical acceptance is verified with a disclosed author-account review process exception. TASK-84 remains open. PR: https://github.com/phdah/sql-semantic-protocol/pull/190. The PR merge is a separate integration gate.

## Immutable scope and evidence

- `docs/task-163-acceptance-freeze-v1.json`: preimplementation frozen fixture identities and eight concrete variant cells (three mandatory Exact, five conditional Residual), with positive/NOT TRUE outcomes, NULL/bag domain requirements, exact expected snapshots, original child/parent AC mapping and owner dispositions.
- `docs/task-163-acceptance-freeze-v2.json`: immutable pinned 104 parser cells (13 dialects × eight transaction/DDL forms), observed via deliberately red CI run https://github.com/phdah/sql-semantic-protocol/actions/runs/42183888762 before freezing the literal results. Active `docs/task-163-acceptance-freeze.json` v3 records technical signoff without changing any original cells. In particular, BigQuery and Snowflake reject generic `BEGIN`; `COMMIT AND CHAIN` may parse but is not a runtime guarantee.
- `tests/task_163_dialect_matrix.rs` asserts the fixed classifications; `tests/task_163_acceptance_inventory.rs` guards immutable v1/v2, v3 case identity, all AC mappings, all conditional owners and the signed-off coverage manifest.
- `tests/task_163_session_laws.rs` checks real DuckDB `~1.10506.0` schema/table-state snapshots for in-transaction CREATE and DROP rollback, a CREATE before BEGIN surviving DML rollback, all row occurrences and NULL frequencies, exact affected counts, typed point-value domains and SELECT TRUE/NOT TRUE ordinals. Invalid COMMIT is Impossible; unproved expression, external CREATE provenance and unsupported session rules are Residual.
- `tests/task_163_adapter_parity.rs` compares identical canonical source bags and typed outcome domains derived from direct SQL schema, authoritative dbt compiled manifest/catalog and ODCS contract evidence (with ODCS feature enabled). None is treated as attestation of engine/session isolation.

## Canonical behavior and fail-closed boundaries

`PhysicalTransactionSessionLaw` is an opt-in, parser-independent requirement on the existing native DuckDB transaction APIs. Only a previously proven complete-catalog DDL schema transition *inside* BEGIN/ROLLBACK can satisfy `DuckDbTransactionalDdlRollback` as Exact. The statement-local graph retains earlier in-script CREATE evidence across SQL units, including exact state/constraint provenance on ROLLBACK; external caller schemas are not trusted CREATE provenance. A REPLACE table generation invalidates the older CREATE declaration. Any prior Impossible stays Impossible; any prior unproved SQL remains Residual at its original step, with TASK-163 ownership appended for requested unproved law.

Conditional release boundaries remain typed Residual rather than fabricated Exact:

| Frozen cell | Boundary | Disposition |
| --- | --- | --- |
| `84-C163-N-temp-table-lifetime` | connection lifetime, temporary storage/scope | TASK-163, unverified |
| `84-C163-N-deferred-fk` | deferred FK timing and native constraint checks | TASK-163, unverified |
| `84-C163-N-concurrent-isolation` | multi-session conflicts and transaction isolation | TASK-163, unverified |
| `84-C163-N-vendor-implicit-commit` | vendor DDL auto-commit semantics, unlike DuckDB | TASK-163, unverified |
| `84-C163-N-vendor-transaction-option` | chain/isolation vendor options and parser-only forms | TASK-163, unverified |

`SAVEPOINT` remains an independently owned TASK-162 native DuckDB Residual. Separately analyzed ALTER metadata propagation remains TASK-164. Neither is silently certified by this child.

## Original child acceptance criteria

| AC | Evidence |
| --- | --- |
| #1 Versioned freeze | v1 retained; v2 pinned 104 parser cells, stable 8 variant IDs and all original AC mappings; exact source/terminal and conditional classifications enumerated |
| #2 Canonical implementation | opt-in typed law API over existing canonical transactional state graph, lifecycle generations and immutable prior CREATE evidence; sound Exact/Impossible/Residual boundaries |
| #3 Native differential oracles | independent DuckDB source/checkpoint/terminal snapshots and row-occurrence, affected-count, NULL domain and SELECT positive/rejected tests |
| #4 All dialects and adapters | 13 × 8 parser classification test plus direct SQL/dbt/ODCS matching schema evidence; no vendor/session law inferred from syntax |
| #5 Final CI and review | Full implementation-head CI https://github.com/phdah/sql-semantic-protocol/actions/runs/42185354954: 9/9 green at `b5814e5261b7b7e2131aaa9fc0b38512e17fa612` (Rust fmt, strict lint all/no-default, full Rust tests, docs, dependencies and dbt E2E). Two-axis review https://github.com/phdah/sql-semantic-protocol/pull/190#pullrequestreview-5797413923: Standards PASS, Spec PASS, no code blockers; author-account review exception disclosed. Final metadata-head CI is a post-signoff PR integration check |

**Review provenance:** The connected-author-account two-axis review is **not** independent second-person review or peer approval. The user's current instruction to finish and sign off TASK-163 is recorded as an owner-directed review-process exception, consistent with prior bounded child signoff; any repository rule requiring a different reviewer remains a merge gate. `independent_review_verified` is explicitly false. Parent TASK-84 remains To Do until TASK-164 and separate original parent acceptance close. No protocol root/schema version change in this additive API.
