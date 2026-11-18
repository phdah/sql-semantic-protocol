# TASK-191 acceptance audit: bounded original exception dispositions signed off

**Owner:** TASK-191, fourth TASK-88 child. **Child status:** Done for exact exception classification only, not parent/release approval.

## Frozen boundary scope

- [Immutable v1 freeze](task-191-acceptance-freeze.json): 55 rejected parser/dialect cells, 116 explicit analyzer Unsupported cells, 23 original native SQL without successful fixed terminal/state.
- [Independent reviewed v2 decisions](task-191-reviewed-dispositions-v2.json): exact unchanged original SQL and 13 dialect IDs, per-cell parser category or precise Unsupported diagnostic (code, message, severity, area), strongest sound Unknown/conditional output and mandatory feature owner. None of the 171 parser/Unsupported review cells is reclassified as supported or an approved deferral.
- [Independent native oracle tests](../tests/task_191_native_original_adjudication.rs): 15 original native invalid/error cases, five stochastic sampling cases, one original unbounded recursion with separately executed bounded companion, one original RANDOM and one optimizer-dependent EXPLAIN. Exact full nullable/duplicate source row identities and unchanged before/after populations are required for the original failure cases. No original successful fixed terminal is fabricated.
- [TASK-179/TASK-197 downstream handoff](task-191-downstream-handoff-v3.json): 194 distinct review obligations mapped to originating SQL, vendor/session and source metadata evidence, and the TASK-180 final owner. Metadata parity is not native execution.

## Acceptance criteria verification

| AC | Child disposition | Executable and review evidence |
| --- | --- | --- |
| 1 | Verified | v1 frozen 55/116/23 identities; v2 source/owner/dialect and complete set reconciliation |
| 2 | Verified | 55 exact source-spelling parser Impossibles and explicit non-native conditional limitations; original public API regression |
| 3 | Verified | 116 exact Unsupported categories/codes/messages, conservative typed Unknown and mandatory feature release blockers |
| 4 | Verified | Full 23 original native error/bounded/stochastic controls and source/poststate checks, plus original/companion distinction |
| 5 | Verified | v3 source-adapter/vendor reconciliation, two-axis review v4, protocol and exact-pinned consumer CI on implementation head |

## Executable verification

**Reviewed implementation SHA:** `44f8f497af57e3f26e82ec5e6c2002111730f24b`  
**Protocol CI:** [9/9 passed](https://github.com/phdah/sql-semantic-protocol/actions/runs/44405445326), covering fmt, strict Clippy, default and no-default Rust tests, docs, dbt Core E2E.  
**Pinned sql-tdg:** [5/5 passed](https://github.com/phdah/sql-tdg/actions/runs/44405560913) in temporary [consumer PR #57](https://github.com/phdah/sql-tdg/pull/57). This temporary CI-only PR must not be merged.

The final acceptance metadata/test-state commit must pass its own CI prior to merging PR #205. See [review v4](task-191-independent-review-v4.json).

## Explicitly unresolved release obligations

Original grammar unsupported in certain parsers is not proof that a vendor runtime rejects equivalent SQL. Parsed but Unsupported mandatory forms remain **release-blocking unimplemented semantics**, not approved Residual feature support. DuckDB errors, stochastic membership and alternate spellings never constitute successful original SQL terminal bags; no native non-DuckDB vendor execution is attested. TASK-180 must independently certify all original mandatory matrix cells; TASK-88 and TASK-91 stay open. TASK-180 does not start before this PR merges.
