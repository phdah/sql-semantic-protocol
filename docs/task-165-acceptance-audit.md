# TASK-165 bounded acceptance and review audit

**Status (2026-11-03):** TASK-165 signed off as Done by the owner, using two explicitly accepted process waivers. PR #182 is ready for final merge checks and remains unmerged. TASK-85 remains open. This is owner authorization, not a separate independent GitHub review.

## Versioned inventory and historical freeze disposition

- `docs/task-165-acceptance-freeze-v1.json` is the immutable eight-case
  inventory originally authored *after* the first semantic slice. TASK-165
  AC #1 required the freeze **before** implementation, so the historical
  ordering is a real process deviation and must not be retroactively relabelled
  as compliant.
- The active v3 freeze `docs/task-165-acceptance-freeze.json` preserves all
  eight stable variant IDs and records executable complete-bag oracles, 13
  parser-dialect identities per case (104 classification cells), evidence-source
  boundaries, typed SQL NULL/integer values, inclusion bounds and a named
  arithmetic-only overflow exception. Archived v2 is preserved. V3 corrects
  the positive grouped fixture source to [0,1] and full terminal [(0,1),(1,1)]:
  these are the **actual** physical key values constructed by the emitted
  `Groups` witness, not merely another SQL example that produces two groups.
  The v3 matrix was frozen before the tightened witness-replay test. No
  mandatory semantic family was weakened by this review correction.
- Release disposition for AC #1 (owner-authorized 2026-11-03): **waiver accepted**.
  A fresh, prospective freeze is impossible for already implemented code.
  Compensating evidence includes immutable v1/v2, active v3, and executable
  case accounting. This waiver closes the process gate without claiming literal
  compliance with the original temporal criterion.

## Evidence per mandatory family

| Case | Independent execution and public API evidence | Boundary |
| --- | --- | --- |
| 85-C165-P-histogram | `tests/task_165_standalone_goals.rs`, `tests/task_165_acceptance_inventory.rs`; physical [1,1,NULL], complete source and output [1,1,NULL], exact SourceRows histogram | SQL NULL share 1/3, non-NULL distinct 1, duplicates 1, closed bag |
| 85-C165-N-total | Same matrix runner; source/output [1,1,NULL] vs impossible requested 2-row/3-bucket goal | Checked total mismatch is unsatisfiable independent of source contents |
| 85-C165-N-distinct | Same matrix runner; physical [1,1,NULL], full terminal [1,NULL] | DISTINCT multiplicity impossibility, **not** a SQL predicate NOT TRUE witness |
| 85-C165-P-group | Frozen physical [0,1] and terminal [(k=0,n=1),(k=1,n=1)]; matrix reconstructs both from the emitted source-key identity, `groups=2`, `rows_per_group=1` | Grouped count = output row count, complete constructive key/group witness |
| 85-C165-N-group | Same matrix runner; physical [1,1,NULL], two groups with counts [2,1] | Two rows with requested three groups is impossible |
| 85-C165-P-zero | Same matrix runner and empty-source test; empty physical/terminal bags | EmptySources witness, 0 numerator, NULL fraction undefined |
| 85-C165-N-overflow | `tests/task_165_standalone_goals.rs` and matrix runner, checked u64::MAX + 1 | Arithmetic-only Impossible. A physically materialized source bag of that size is impossible to execute, so no native engine claim is made |

The matrix runner reads the task-owned JSON (rather than recomputing
expectations from the implementation), checks every mandatory dialect-cell
verdict and canonical witness tag, reconstructs physical inputs and terminal
bags from `SourceRows` and `Groups` witnesses, inserts the independently frozen
materializable physical occurrences into DuckDB, and compares the *entire*
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

## TASK-165 acceptance status mapped to parent TASK-85

| Child AC | Parent original AC | Implementation result | Sign-off |
| --- | --- | --- | --- |
| #1 Frozen variants and prospective timing | #1, #3, #5, #6 | v1/v2 archived; active v3 froze the corrected group witness before its test. The original v1 was late and cannot become prospectively frozen by editing history | **Closed by explicit owner waiver (2026-11-03), historical deviation retained** |
| #2 Canonical complete-bag semantics | #1, #3 | Checked typed histogram totals, exact NULL/distinct/duplicate rates, full SourceRows and Groups witness bags; contradiction remains Impossible | **Technical pass** |
| #3 Independent physical DuckDB oracle | #5, #6 | Positive/negative [1,1,NULL] and group [0,1] source plus full terminals; impossible overflow arithmetic only | **Technical pass** |
| #4 Dialects/adapters/engine boundary | #1, #6 | 91 mandatory dialect cells and 13 conditional residual cells, direct SQL/dbt catalog/ODCS parity, DuckDB only as native engine | **Technical pass** |
| #5 Final CI, pinned consumer and independent review | #5, #6 | Previous head [CI 41531283571](https://github.com/phdah/sql-semantic-protocol/actions/runs/41531283571) 9/9 green; 69 pinned consumer selected tests pass and two timestamp baseline failures unchanged. Final bookkeeping-only commit CI must be checked before merge; no distinct reviewer approval exists | **Closed by explicit owner waiver of distinct-reviewer identity (2026-11-03)** |

These mapped parent AC are **evidence links**, not parent TASK-85 completion.
Child AC #2/#3/#4 are verified on implementation evidence; #1/#5 have now
been closed via **owner-authorized exceptions**, explicitly accepted on
2026-11-03. A waived requirement is not asserted to have been literally
satisfied. No independent GitHub approver is claimed.

## Matt code-review axes against `main...HEAD`

### Standards

- Code stays in `src/outcome_goals.rs` with checked arithmetic and typed
  public accessors; no dependency, `unsafe`, protocol wire change or adapter
  owned semantic duplicate.
- Public integration tests use supported API seams and independent DuckDB bags.
- Potential style concern: a separate statistic type is a deliberately small
  value object for repeated NULL/distinct/duplicate counts, not a second
  authoritative protocol representation.
- **Reviewer-identity exception: owner accepted (2026-11-03).** The Matt
  two-axis review was submitted through the author-connected GitHub account;
  it is not an independent GitHub approval. The owner explicitly waived the
  distinct-reviewer-identity gate for TASK-165 only.

### Spec

- The seven mandatory case semantics are linked to explicit golden whole-bag
  and Impossible oracles; a conditional no-schema residual is named.
- Both originally outstanding process gates have explicit owner dispositions:
  AC #1 late freeze accepted as a waiver; AC #5 independent identity waived.
  The seven mandatory families (91 parser cells) were evaluated in the Matt
  two-axis review and by the frozen DuckDB/adapter acceptance tests. The
  original unfulfilled historical conditions remain visible.
- Pinned `sql-tdg` tests were run against an implementation SHA, with the same
  public API and protocol semantics as the present PR head; two pre-existing
  timestamp-specific failures were separately compared to the original base.
  Broader consumer release certification remains owned by TASK-89/91.
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
green must not be claimed. These are **explicitly deferred to TASK-89/91** as unchanged, unrelated
consumer compatibility failures; they are not treated as success or as a new
TASK-165 semantics bug. TASK-165 claims only the verified selected downstream
suites, not a fully green downstream repository.

The temporary CI workflow was removed from the implementation branch
after capturing the qualification run, so the protocol repository keeps
no permanent dependency on the consumer's Git SHA.

## CI and merge gate

The GitHub CI workflow runs fmt, strict all-target Rust Clippy, tests, docs,
no-default-features lint/test/docs and dependency guard, and compiled dbt E2E.
Implementation and prior metadata head `98f87c940e8f6dc276970eb4a847493a06e3dace`
passed all nine jobs: [CI 41531283571](https://github.com/phdah/sql-semantic-protocol/actions/runs/41531283571).
Pinned downstream evidence is recorded above. With both owner waivers accepted,
TASK-165 is Done and PR #182 remains open, non-draft. Check current-head CI for
this audit-only update and GitHub mergeability before squash-merging. The parent
TASK-85 remains open; no distinct independent reviewer approval is claimed.
