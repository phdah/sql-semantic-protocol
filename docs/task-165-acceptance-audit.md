# TASK-165 bounded acceptance and review audit

**Status:** Implementation in PR #182 is pending final-head verification and
independent acceptance sign-off. TASK-85 remains open. This audit deliberately
does not claim TASK-165 Done from passing tests alone.

## Versioned inventory and historical freeze disposition

- `docs/task-165-acceptance-freeze-v1.json` is the immutable eight-case
  inventory originally authored *after* the first semantic slice. TASK-165
  AC #1 required the freeze **before** implementation, so the historical
  ordering is a real process deviation and must not be retroactively relabelled
  as compliant.
- The active v2 freeze `docs/task-165-acceptance-freeze.json` preserves all
  eight stable variant IDs and records executable complete-bag oracles, 13
  parser-dialect identities per case (104 classification cells), evidence-source
  boundaries, typed SQL NULL/integer values, inclusion bounds and a named
  arithmetic-only overflow exception. No mandatory SQL semantics have been
  weakened by v2.
- Release disposition for AC #1: **requires independent reviewer decision**.
  A fresh, prospective freeze is impossible for already implemented code.
  Compensating evidence includes immutable v1, v2 delta and executable case
  accounting. This does not automatically satisfy the temporal criterion.

## Evidence per mandatory family

| Case | Independent execution and public API evidence | Boundary |
| --- | --- | --- |
| 85-C165-P-histogram | `tests/task_165_standalone_goals.rs`, `tests/task_165_acceptance_inventory.rs`; physical [1,1,NULL], complete source and output [1,1,NULL], exact SourceRows histogram | SQL NULL share 1/3, non-NULL distinct 1, duplicates 1, closed bag |
| 85-C165-N-total | Same matrix runner; source/output [1,1,NULL] vs impossible requested 2-row/3-bucket goal | Checked total mismatch is unsatisfiable independent of source contents |
| 85-C165-N-distinct | Same matrix runner; physical [1,1,NULL], full terminal [1,NULL] | DISTINCT multiplicity impossibility, **not** a SQL predicate NOT TRUE witness |
| 85-C165-P-group | Same matrix runner and grouped public API test; physical [1,2], two group terminal rows | Grouped count = output row count, complete key-group witness |
| 85-C165-N-group | Same matrix runner; physical [1,1,NULL], two groups with counts [2,1] | Two rows with requested three groups is impossible |
| 85-C165-P-zero | Same matrix runner and empty-source test; empty physical/terminal bags | EmptySources witness, 0 numerator, NULL fraction undefined |
| 85-C165-N-overflow | `tests/task_165_standalone_goals.rs` and matrix runner, checked u64::MAX + 1 | Arithmetic-only Impossible. A physically materialized source bag of that size is impossible to execute, so no native engine claim is made |

The matrix runner reads the task-owned JSON (rather than reproducing an
independent expectation from the implementation), checks every mandatory
dialect-cell verdict and canonical witness tag, inserts all materializable
frozen source occurrences into DuckDB, and compares the *entire*
source and terminal bag including duplicate and NULL occurrences. Full
native execution is a DuckDB oracle, **not** a claim that 13 vendor engines ran.

## Adapter parity and conservative unknowns

`tests/task_165_adapter_parity.rs` compares equivalent source catalog
evidence across direct SQL, compiled dbt model SQL with **warehouse catalog**
types, and ODCS physical INTEGER schema evidence. The direct, DISTINCT and
ordinary grouped shapes retain the same feasible/impossible classifications.
No adapter invents a physical bag or SQL execution from metadata.

`tests/task_165_acceptance_inventory.rs` verifies that identical SQL
without authoritative source datatype evidence remains Residual on all
13 dialect parser paths. An unattested native collation or implicit cast is not
equivalent to canonical typed bucket identity; external engine truth requires
separate attestation.

## Matt code-review axes against `main...HEAD`

### Standards

- Code stays in `src/outcome_goals.rs` with checked arithmetic and typed
  public accessors; no dependency, `unsafe`, protocol wire change or adapter
  owned semantic duplicate.
- Public integration tests use supported API seams and independent DuckDB bags.
- Potential style concern: a separate statistic type is a deliberately small
  value object for repeated NULL/distinct/duplicate counts, not a second
  authoritative protocol representation.
- **Independent review status: pending.** A self-audit does not constitute a
  separate human review; final-head Clippy/fmt/test outcomes are recorded by CI.

### Spec

- The seven mandatory case semantics are linked to explicit golden whole-bag
  and Impossible oracles; a conditional no-schema residual is named.
- Two genuine outstanding gates: AC #1 historical ordering needs explicit
  reviewer disposition; AC #5 needs final-head checks and an independently
  signed case-by-case review.
- Relevant pinned `sql-tdg` consumer tests have not been run against the
  PR head. Earlier protocol-owned conformance tests are not a substitute;
  see TASK-99/100 and the separate downstream TASK-89/91 gates.
- Parent TASK-85 original AC #1/#3/#5/#6 remain open until the later
  children and independent parent certification complete.

## Pinned downstream consumer evidence

The one-off qualification run
[41522367446](https://github.com/phdah/sql-semantic-protocol/actions/runs/41522367446)
checked pinned `phdah/sql-tdg@3beaf7799ab54b2440ae34c917ff7eda6c11c398`
against the PR protocol head `6e767dd62ef37a1ab5d30ddd515f7f423fe7503b`.
The consumer's 34 library unit tests, 2 type-coverage tests,
26 full-query tests and 7 exactness tests passed. Two timestamp-specific
tests were separately executed against both the pre-PR baseline
`7af4c128d5bc5785982ccdcd6bc7fed603651938` and this PR:
`full_query/protocol_driven_timestamp_generation` and
`exactness/timezone_dependent_timestamp_filters_require_session_assumption`.
Both returned exit 101 at baseline **and** PR head, with the identical
`reason=computed_expression clause=where` residual diagnostic. They are
pre-existing compatibility failures, not TASK-165 regressions. Full-suite
green must not be claimed. They remain explicit conditional consumer
exceptions pending the separate consumer/release owner disposition.

The temporary CI workflow was removed from the implementation branch
after capturing the qualification run, so the protocol repository keeps
no permanent dependency on the consumer's Git SHA.

## CI and merge gate

The GitHub CI workflow runs fmt, strict all-target Rust Clippy, tests, docs,
no-default-features lint/test/docs and dependency guard, and compiled dbt E2E.
Final-head evidence URL, SHA, job verdict and downstream pin must be written
here after completion. Leave PR #182 in Draft and TASK-165 In Progress until
the mandatory cells, documented residuals, independent review, pinned
consumer test and explicit freeze deviation disposition have been signed off.
