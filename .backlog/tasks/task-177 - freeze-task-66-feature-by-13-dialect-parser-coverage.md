---
id: TASK-177
title: Freeze TASK-66 feature-by-13-dialect parser coverage
status: In Progress
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-09'
labels: []
milestone: m-3
dependencies:
  - TASK-66
  - TASK-70
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-77
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
references:
  - TASK-88
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-88 child 1/4.** Freeze TASK-66 feature-by-13-dialect parser coverage. Implement this scope in its own PR against `main` after dependencies merge. Preserve original TASK-88 AC for separate parent signoff. Never create parent-child dependency cycles.
<!-- SECTION:DESCRIPTION:END -->

## Bounded planning scope

- **Stable fixture IDs:** `88-C177-P`, `88-C177-N`.
- **Finite variants:** 13 named dialects; every reviewed variant; parser accepted/unsupported statuses.
- **Independent oracle/verification baseline:** Full cross-product of approved stable TASK-66 variant IDs with 13 dialects; no missing rows or implicit skip.
- **Conditional/external evidence boundary:** Parser success alone never certifies outcome semantics or vendor engine execution. Owner TASK-177; release disposition pending review.
- **Original parent AC:** #1, #2, #4, #5, #7, #8.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-88 stays open until all children and original AC pass.
<!-- AC:END -->

## Next

Next: [TASK-178](task-178%20-%20certify-canonical-equivalence-for-shared-dialect-ast-variants.md), only after completion and merge.

## Implementation evidence (2026-11-07, not final sign-off)

- `docs/task-177-acceptance-freeze.json` v1 freezes the entire TASK-66
  251-variant x 13-dialect inventory (3,263 named cells) and explicitly retains
  the 3,002 unverified, 105 representative-only and 156 fixture-tested parser
  statuses. Sparse overrides are authoritative; family-level fixture evidence
  is not inherited by other variants.
- `tests/task_177_dialect_inventory.rs` checks complete coverage, individual
  fixture provenance, no invented SQL, no implicit dialect skips and the
  conservative absence of canonical/native-execution claims. Named
  `88-C177-P` and `88-C177-N` oracles require complete physical source,
  selected, NOT TRUE and UNKNOWN bags and compare the same portable SQL
  through all 13 public analysis dialects.
- The new machine-readable registry is linked by
  `docs/coverage-manifest.json.task_177_dialect_parser_acceptance`.
- This is an **in-progress bounded evidence freeze**, not acceptance sign-off.
  Missing exact SQL shapes, typed domain/source/terminal outcomes for the
  remaining variant/dialect cells, direct SQL/dbt/ODCS parity, native
  vendor/session certification, full CI, independent review and pinned-consumer
  gates remain blocking. All original AC #1..#5 stay unchecked, TASK-88
  remains open, and TASK-178 stays dependency-blocked until this child is
  fully certified, Done and merged.

## Parser sample inventory extension (2026-11-07)

- `docs/task-177-parser-samples.json` adds 251 task-owned sample SQL
  statements, one per reviewed TASK-66 feature variant.
- `docs/task-177-parser-snapshot.json` records independent CI-observed
  classifications for the 3,263 example SQL/dialect pairs:
  **3,208 parsed, 55 parser-rejected**. The 55 sample rejections belong to
  12 named variant families. No silent dialect skip or vendor execution
  claim is permitted.
- `docs/task-177-parser-boundaries.json` names all 55 rejected
  sample/dialect pairs, with conditional ownership and *unapproved*
  release disposition. Alternative native syntax requires independent
  evidence, not automatic fallback or claimed impossibility.
- `docs/task-177-acceptance-audit.md` maps all five original AC
  individually and enumerates remaining mandatory semantic, typed physical,
  SQL/dbt/ODCS, native session, consumer and independent-review gates.
  **Do not mark the child Done merely because parser and CI tests pass.**

## Public API analysis classification (2026-11-07)

A CI-observed second fixed snapshot distinguishes parser boundary from the
protocol library's own interpretation for all 3,263 sample/dialect pairs:
3,092 protocol document returns, 116 explicit Unsupported statements, 55
parser rejections, zero analysis errors. The `docs/task-177-analysis-boundaries.json`
registry assigns those 116 cases to original semantic owners, retains
unapproved blocking disposition and refuses to promote returned documents
to certified typed domains or native engine proofs. Original AC remain
unchecked, pending the independent whole-bag and adapter equivalence
requirements documented in `docs/task-177-acceptance-audit.md`.


## Source-complete predicate and adapter evidence (2026-11-08)

- `docs/task-177-predicate-oracles.json` pins 25 original TASK-66
  predicate samples (S001..S019, S027..S032) to 11 source identities, a
  seven-occurrence customer source and expected complete terminal and
  TRUE/FALSE/UNKNOWN source bags; no source rows are dropped from NOT TRUE.
- `tests/task_177_predicate_oracles.rs` checks each native DuckDB whole
  bag and six typed inclusion-set or interval bounds, comparing their
  canonical emissions across all 13 portable parser dialects.
- `tests/task_177_predicate_adapter_parity.rs` tests six direct SQL,
  dbt compiled manifest and representable ODCS typed source facts,
  preserving `ProducerExecution::Unattested` for dbt.
- This reduces but does **not close** the remaining original AC: full
  semantic and physical oracles are absent for other variants, 55 parser
  and 116 explicitly Unsupported cells lack approved dispositions,
  and pinned consumer plus independent per-cell signoff remain pending.

## Complete public canonical dialect differential and native drift oracles (2026-11-08)

- The versioned `docs/task-177-equivalence-snapshot.json` fixes all 251
  first-party samples across the 13 project dialect parsers, using complete
  public canonical JSON (minus the explicit input dialect metadata). There
  are 3,069 identical document cells, 23 different cells, 116 explicit
  Unsupported cells, 55 parser-rejected cells, and zero analysis errors.
- `docs/task-177-canonical-drift-boundaries.json` owns all 23 actual
  complete-public-protocol differences across ROLLUP, CUBE, GROUPING(),
  correlated LATERAL and STRUCT CAST. Exact first differing JSON paths,
  source task owners and unapproved review dispositions are pinned.
- `tests/task_177_cross_dialect_native_oracles.rs` executes original SQL
  in DuckDB against complete nullable, duplicate source bags, verifying
  ROLLUP/CUBE subtotal multiplicities, GROUPING source NULL identity,
  LATERAL identity preservation and STRUCT scalar fields. It provides five
  native original-SQL oracles in addition to the earlier 27 P/N cases.
- **Still In Progress:** Cross-dialect structural agreement is not the
  strongest exact typed-domain or physical outcome proof. The full mandatory
  cell matrix, vendor/session laws, remaining representable adapter facts,
  candidate-pinned sql-tdg verification and independent acceptance/release
  reviewer remain outstanding. AC #1..#5 remain unchecked, TASK-88 open and
  TASK-178 blocked pending separately reviewed child completion.

## Independent native evidence expansion (2026-11-08, still In Progress)

- The new `docs/task-177-relational-bag-oracles.json` and
  `tests/task_177_relational_bag_oracles.rs` pin and execute 98 original
  SQL samples with whole-source NULL/duplicate and full-terminal DuckDB
  bags. Nine family tests span joins, set operations, grouping, windows,
  subqueries, scopes, pagination and advanced table sources.
- Three other frozen example spellings (LEFT SEMI/ANTI and FETCH FIRST
  WITH TIES) are native DuckDB parser rejections; separately verified
  fixture-specific alternatives do not promote these examples to native
  exact SQL. UNION BY NAME executes natively but its canonical output
  composition is explicitly unsupported with diagnostics.
- `docs/task-177-physical-proof-ledger.json` and its executable test now
  enumerate all 251 cases: **125 distinct** original SQL examples have
  native first-party source/terminal oracles (including earlier predicate
  and drift evidence), **126 remain missing**, and only **six** have exact
  typed SQL/dbt/ODCS-domain parity evidence. The two named standalone P/N
  goldens do not count toward the 251.
- **Signoff remains blocked:** Full strongest-domain, positive/rejected
  whole-bag, per-adapter, conditional dialect/vendor/session and independent
  per-cell reviews remain incomplete. AC #1..#5 are not checked, TASK-178
  remains dependency-blocked and TASK-88 remains open.

## Additional native original-SQL proof (2026-11-09, still In Progress)

- Five additional task-owned, versioned oracle fixtures and public DuckDB
  test suites cover **89 more distinct original SQL samples**: query, sorting,
  PIVOT and multi-terminal bags; duplicate/NULL write before/after states;
  timestamp, string and signed-zero environments; schema/DDL, constraints
  and rollback; and ranking/window frame outcomes.
- `tests/task_177_physical_proof_ledger.rs` now reconciles original owners
  and guards **214 of 251** sample IDs with first-party native source and
  terminal evidence, **37** without native complete-bag or state proof.
  Separate fixed native rejection tests do not inflate this count.
- The additional original SQL executions do not certify independent strongest
  typed-domain outcomes for all 3,263 cells or all equally representable
  SQL/dbt/ODCS facts, and do not certify native vendor/session equivalence.
  The 23 canonical drifts, 55 parser rejects, 116 analyzer Unsupported cells
  and independent per-cell release dispositions remain unresolved.
- **No signoff:** All original AC #1..#5 stay unchecked; TASK-177 remains
  In Progress and PR #196 stays draft until mandatory proofs and independent
  review are complete.

## Latest deterministic native and bounded control evidence (2026-11-09, still In Progress)

- CI-confirmed original SQL deterministic tests now cover 13 additional distinct source/terminal or poststate examples; the case-level proof ledger records **227 of 251** with native evidence and **24** original SQL examples remaining without a complete native terminal/state proof.
- New native control tests exercise frozen rejected SQL with complete source setups, sample only source occurrences, and bound the recursive/RANDOM controls without inventing exact stochastic or nonterminating terminal outputs.
- None of this certifies strongest typed outcome domains for all parser cells, cross-adapter parity, vendor/session equivalence, approved conditional laws or independent signoff. Original AC #1..#5 remain unchecked; TASK-178 is still blocked.
