# TASK-120 independent acceptance audit

Date: 2026-10-19
Scope: TASK-120 child 4/4 of TASK-72
Review head: `2605162c9fd07e1a0a27767cf368b8b86af99e79`
Reviewed passing CI: [nine of nine jobs](https://github.com/phdah/sql-semantic-protocol/actions/runs/39250351880)
Acceptance freeze: [version 3](task-120-acceptance-freeze.json)
Status: child accepted, pending final metadata-head CI and merge; **TASK-72 remains open**

## Acceptance criteria independently reviewed

1. **Frozen finite matrix, complete.** Exactly 21 mandatory portable cases and six conditional cases, fixed fixture IDs `72-C120-P` and `72-C120-N`, stable source identities, SQL, full source and terminal tuple populations, typed value domains, nullable/empty cases and interval bounds. All six original TASK-72 criteria are mapped, with explicit per-case dialect and adapter cells. Audited the v3 correction: no numeric interval means no inclusivity flag, and conditional unproven output bags are `null`, never false claims of zero rows.
2. **Canonical exact semantics, complete within frozen mandatory scope.** The public physical API proves all six two-column `UNION`, `INTERSECT`, `EXCEPT` DISTINCT/ALL laws, NULL-sensitive multisets, nested/shared-source branches, and total-order LIMIT/OFFSET/FETCH cases. The tests reject incomplete inputs as `Residual`, enforced NULL violations as `Impossible`, and require exact tuple frequencies and typed outcome domains for mandatory `Exact` proofs, including complete TRUE/FALSE/UNKNOWN source partitions where applicable.
3. **Independent executable oracle, complete.** `tests/task_120_parent_set_matrix.rs` loads frozen inputs into DuckDB, checks complete physical `(id,k,v)` source snapshots and complete terminal two-column histograms (not only row counts). The expected snapshots are frozen separately from the semantic evaluator. It verifies both P and N cases, including empty, duplicate, NULL, and rejected-source counterexamples.
4. **Dialect and adapter parity, complete for mandatory portable evidence.** All 21 mandatory cases have exact typed and source-backed physical results in each of the 13 exposed parser dialects (273 cases). All 21 mandatory cases must produce the same complete bag and output domains through direct SQL, dbt compiled-artifact analysis and ODCS-equivalent schema metadata (63 adapter-case proofs). The six conditional cases have 78 recorded parser/session boundary classifications; unsupported BY NAME/CORRESPONDING, coercion, branch-local limits, tie ordering and MINUS dialect semantics remain explicitly conditional. Parser equivalence and DuckDB execution are not native vendor execution proofs.
5. **Final implementation CI and scope audit, complete.** All nine jobs succeeded at the reviewed implementation head: default and no-default full Rust tests, strict lint, formatting, both docs, no-default dependency check, and dbt Core E2E. [Run 39250351880](https://github.com/phdah/sql-semantic-protocol/actions/runs/39250351880). Golden/contract regression tests execute as part of full Rust tests; this test-only change does not alter public protocol JSON/schema. The task closure metadata commit requires its own final-head green CI before merging.

## Non-promotions and parent gate

- All 21 mandatory portable cases are exact; there are no accepted mandatory `Residual` cases.
- Six conditional cases retain named TASK-120 ownership and their recorded release disposition; they cannot be mistaken for certified empty outputs, or upgraded to native vendor-session claims without an independent engine oracle.
- Sign-off applies **only to TASK-120**. TASK-72 still requires review of its original #1..#6 criteria against all merged children, and TASK-91 separately guards the release. No parent or release approval is implied.
