---
id: TASK-175
title: Resolve schema aliases FK-only source and adapter limitations
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-07'
labels: []
milestone: m-3
dependencies:
  - TASK-174
references:
  - TASK-87
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-87 child 3/4.** Resolve schema aliases FK-only source and adapter limitations. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-87 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `87-C175-P`, `87-C175-N`.
- **Finite variants:** schema-qualified quoted names; tests; FK-only evidence; missing warehouse metadata.
- **Independent oracle/verification baseline:** same source DDL/ODCS/dbt key and schema produce same typed canonical domain; missing FK target yields explicit diagnostic.
- **Conditional/external evidence boundary:** No fabricated warehouse keys, catalog introspection or test enforcement. Owner TASK-175; release disposition pending review.
- **Original parent AC:** #1, #2, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [x] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [x] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [x] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [x] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-87 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

Next: [TASK-176](task-176%20-%20certify-sql-dbt-odcs-paired-canonical-parity-across-writes.md), only after completion and merge.

## Implementation (2026-11-07)

- Acceptance matrix frozen before semantic changes: `docs/task-175-acceptance-freeze.json` v1, registered in `docs/coverage-manifest.json`.
- TASK-87 parent and TASK-91 release gates stay open; mandatory CI and review are not preapproved.

## Bounded TASK-175 technical signoff (2026-11-07)

- **Original child AC #1-5:** technically verified and checked for the original eight cases (six mandatory, two owned conditional). Exact physical source/terminal bag/NULL/duplicate, quoted names, FK-only typed schema, unknown `to` diagnostic and schema/constraint parity are independently asserted. TASK-87 parent AC are not closed.
- **Evidence:** `docs/task-175-acceptance-freeze.json` v2 (immutable original v1 archived), `docs/task-175-acceptance-verification.json`, `docs/task-175-acceptance-audit.md`, and `tests/task_175_adapter_boundaries.rs`. The active version adds 78 portable parser-dialect and 19 adapter cells without changing original case identities.
- **CI:** all 9 implementation-head checks passed at `91ab73aadaf7f804ab2c8d0379b37e81a07cbc5c`: https://github.com/phdah/sql-semantic-protocol/actions/runs/42376329382 (fmt, strict lint, full/no-default Rust tests and docs, dependencies, dbt E2E).
- **Review:** two-axis standards/spec PASS under explicit same-account owner-directed review-process exception: https://github.com/phdah/sql-semantic-protocol/pull/194#pullrequestreview-5812435290. This is **not** independent second-human peer review. Historical v1 freeze lacked complete per-cell detail; v2 retrospective evidence is documented, not represented as retroactive pre-freeze compliance.
- **Consumer disposition:** sql-tdg pins published protocol 2.0.3 with `default-features = false`, so its current pinned tests cannot exercise this unreleased dbt-only adapter change. Consumer v3 acceptance remains independently gated by TASK-91.
- **Integration:** PR #194 remains separate merge gate. This bounded child Done state does not unblock TASK-176 until PR #194 is merged on `main`. No parent or release gate is approved.
