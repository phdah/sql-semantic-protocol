---
id: TASK-176
title: Certify SQL dbt ODCS paired canonical parity across writes
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-07'
labels: []
milestone: m-3
dependencies:
  - TASK-175
references:
  - TASK-87
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-87 child 4/4.** Certify SQL dbt ODCS paired canonical parity across writes. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-87 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Implementation (2026-11-07)

- Freeze v1 registered in `docs/task-176-acceptance-freeze.json` and `docs/coverage-manifest.json` before code changes.
- TASK-87 parent and TASK-91 release gates remain open; child AC and verification gates are not preapproved.

## Bounded planning scope

- **Stable fixture IDs:** `87-C176-P`, `87-C176-N`.
- **Finite variants:** three adapter pairs: table, view/ephemeral, incremental/write; typed source constraints.
- **Independent oracle/verification baseline:** Compare full JSON facts/physical proof strengths on equivalent evidence and retain intentional materialization differences.
- **Conditional/external evidence boundary:** Every newly exposed adapter fact must have exact vs residual assertions and full CI. Owner TASK-176; release disposition pending review.
- **Original parent AC:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [x] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [x] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [x] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [x] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-87 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

After this child, audit and sign off TASK-87 separately; TASK-91 controls release readiness.

## Bounded TASK-176 technical signoff (2026-11-07)

- **Child AC #1-5:** signed off with explicit process exceptions. Original v1 freeze was recorded before code, but its write case had an incorrect read oracle; immutable v1 is archived and corrected v2 was reviewed retrospectively. This is not retroactive prefreeze compliance.
- **Executable oracles:** `tests/task_176_adapter_write_parity.rs` covers seven mandatory cases, one reviewed conditional boundary, all 65 named 13-dialect parser cells and 15 direct/dbt/ODCS adapter cells. It asserts canonical emitted JSON fields, exact typed source/terminal bags, NULL/duplicates, TRUE/NOT TRUE, MERGE/DELETE state, inclusive domain bounds, and distinct Impossible/Residual.
- **Code-head CI:** 9/9 checks passed at `52f65e8674213c53d1ce1c20cb3ebc4cbe308d45`: https://github.com/phdah/sql-semantic-protocol/actions/runs/42397057115 (fmt, strict Clippy, full/no-default Rust tests/docs, dependency checks, dbt E2E). Final metadata-head CI remains a separate integration check.
- **Review:** Standards/spec PASS under the owner-directed same-account review exception: https://github.com/phdah/sql-semantic-protocol/pull/195#pullrequestreview-5814037915. This is **not** independent second-human peer review, and organizational branch protection is not waived.
- **Consumer:** `phdah/sql-tdg` pins published 2.0.3 with no default features. This certification is test-only and cannot validate an unreleased dbt/ODCS behavior through that published pin; v3 release consumer tests remain TASK-91.
- **Integration:** PR #195 remains an unmerged gate. TASK-87 parent AC must be independently reviewed; TASK-91 is not approved.
