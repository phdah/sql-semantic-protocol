---
id: TASK-164
title: Certify ordered mixed script state and final targets
status: In Progress
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
- [ ] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [ ] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [ ] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [ ] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-164 and reviewed disposition.
- [ ] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-84 open.
<!-- AC:END -->

## Sequencing

Then independently verify all original TASK-84 AC and parent signoff; TASK-91 is a separate gate.

## Implementation evidence (2026-11-06)

- Bounded v3 acceptance inventory: `docs/task-164-acceptance-freeze.json`, preserving preimplementation v1 and corrected pre-matrix v2 in immutable `docs/task-164-acceptance-freeze-v1.json` and `docs/task-164-acceptance-freeze-v2.json`; 598 parser statement cells are pinned before final technical signoff.
- Canonical typed ALTER-default propagation through transaction checkpoints: `src/physical_realization/transactions.rs`.
- Full DuckDB, negative, NULL/duplicate and rollback oracles: `tests/task_164_mixed_scripts.rs`; equivalent SQL/dbt/ODCS source metadata: `tests/task_164_adapter_parity.rs`; 13 parser dialect classifications: `tests/task_164_dialect_matrix.rs`.
- Status remains In Progress until final-head CI, complete mandatory oracle verification and an independent two-axis review. TASK-84 parent remains open. The TASK-162 native SAVEPOINT and TASK-163 vendor/session boundaries remain Residual.
