---
id: TASK-164
title: Certify ordered mixed script state and final targets
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-06'
labels: []
milestone: m-3
dependencies:
  - TASK-163
references:
  - TASK-84
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-84 child 4/4.** Implement certify ordered mixed script state and final targets in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-84.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `84-C164-P`, `84-C164-N`.
- **Named mandatory/conditional variants:** mixed DDL,DML,MERGE/read; output counts; dbt/ODCS; dialects.
- **Numerical/row-state oracle:** CREATE -> INSERT 2 -> MERGE 1 update -> DELETE 1 -> SELECT yields 1 row; every transition snapshotted.
- **Typed conditional boundary:** No CI green claim until all intermediate and rollback paths have executable oracles. Owner: TASK-164.
- **Original TASK-84 criteria:** #1, #2, #3, #4, #5, #6.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-164 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-84 open.
<!-- AC:END -->

## Sequencing

Then independently verify all original TASK-84 AC and parent signoff; TASK-91 is a separate gate.

## Implementation evidence (2026-11-06)

- Bounded v6 bounded technical acceptance: `docs/task-164-acceptance-freeze.json`, immutable v1-v5 archived. Eight original cases (five Exact, one Impossible, two conditional Residual) and 598 dialect parser cells retain the same original case identities, with complete SQL/dbt/ODCS and native DuckDB full-bag/typed domain evidence.
- Canonical typed ALTER-default propagation through transaction checkpoints: `src/physical_realization/transactions.rs`.
- Full DuckDB, negative, NULL/duplicate and rollback oracles: `tests/task_164_mixed_scripts.rs`; equivalent SQL/dbt/ODCS source metadata: `tests/task_164_adapter_parity.rs`; 13 parser dialect classifications: `tests/task_164_dialect_matrix.rs`.
- All five original child AC are technically signed off under the disclosed owner-directed connected-author-account review exception. This is not a claim of independent second-human review. TASK-84 parent remains To Do and TASK-91 release remains gated; the TASK-162 native SAVEPOINT and TASK-163 vendor/session laws remain explicitly Residual.

## Bounded child acceptance signoff (2026-11-06)

- **Status:** TASK-164 Done for its bounded eight-case freeze v6, retaining immutable v1-v5, with the requested owner-directed author-account review-process exception disclosed. The original TASK-84 parent AC #1-6 remain open for separate signoff.
- **PR:** https://github.com/phdah/sql-semantic-protocol/pull/191. Merge is a separate integration gate; no automatic merge.
- **Scope:** Five mandatory Exact outcomes, one mandatory Impossible duplicate CREATE, two owned conditional Residual boundaries (TASK-162 SAVEPOINT; TASK-163 vendor/session DDL). Fixed fixture IDs `84-C164-P`, `84-C164-N`. All 598 parser-statement cells across 13 dialects pinned. SQL/dbt/ODCS parity covers only representable canonical schema/SQL evidence, not inferred runtime session laws.
- **Oracles:** Every Exact checkpoint has an independently frozen complete live catalog, column schema/type, full duplicate/NULL bag, typed integer/NULL frequencies and inclusive bounds, affected mutation counts and source-row identity/TRUE/NOT TRUE ordinals. Independent DuckDB execution, rollback, invalid CREATE and ALTER-default generation are asserted in `tests/task_164_mixed_scripts.rs`; dialects and adapter parity in `tests/task_164_dialect_matrix.rs` and `tests/task_164_adapter_parity.rs`.
- **Verified implementation-head CI:** https://github.com/phdah/sql-semantic-protocol/actions/runs/42205848985 at `eaf6ef2aa4607c58bf41b61e9f0288a37c643404`, 9/9 green: fmt, strict lint both feature configurations, full/no-default Rust tests, docs, dependency and dbt E2E. Protocol root/schema unchanged.
- **Two-axis review:** https://github.com/phdah/sql-semantic-protocol/pull/191#pullrequestreview-5799127718; Standards PASS, Spec PASS, no blockers. Submitted with the connected PR author's GitHub account, **not independent human peer review**. Current owner instruction requested child signoff with the same transparently disclosed process exception used for TASK-161..163; branch protection can still require another reviewer.
- **Historical freeze timing deviation:** The pre-semantic v1 freeze declared the finite eight cases but not all post-hoc strict typed/checkpoint/adapter fields. Later immutable v2-v5 and active v6 strengthened only evidence, preserving case identity, SQL programs and mandatory classifications. The original procedural gap is not represented as retroactively fixed; owner-directed signoff acknowledges it.
- **Release disposition:** TASK-84 parent remains To Do; TASK-91 release gate is not approved. No engine-specific vendor or SAVEPOINT unsupported behavior is promoted from Residual to Exact.
