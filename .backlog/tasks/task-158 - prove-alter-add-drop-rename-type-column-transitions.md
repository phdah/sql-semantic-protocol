---
id: TASK-158
title: Prove ALTER ADD DROP RENAME TYPE column transitions
status: Done
assignee: []
created_date: '2026-10-17'
updated_date: '2026-11-02'
labels: []
milestone: m-3
dependencies:
  - TASK-157
references:
  - TASK-83
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**TASK-83 child 2/4.** Implement prove alter add drop rename type column transitions in a focused PR against `main` after dependencies merge. The parent original AC are additive and remain open until independently signed off. Never depend on parent TASK-83.
<!-- SECTION:DESCRIPTION:END -->

## Finite execution contract

- **Stable fixture IDs:** `83-C158-P`, `83-C158-N`.
- **Named mandatory/conditional variants:** ALTER TABLE ADD/DROP/RENAME/TYPE; defaults; data preservation; key checks.
- **Numerical/row-state oracle:** table (a=1) ADD b DEFAULT 2 => row (1,2); RENAME a TO c => (c=1,b=2).
- **Typed conditional boundary:** Engine-specific cast/rewrite failure is typed residual with before/after schema. Owner: TASK-158.
- **Original TASK-83 criteria:** #2, #4, #5, #6.

## Implementation progress (2026-11-01)

PR #179 includes SQL AST `ALTER TABLE` normalization, a canonical `schema_alter` write kind with ordered typed operations, and complete-bag replay for conservative ADD/DROP/RENAME/identical-TYPE and provably lossless integer widening transitions. `PhysicalWriteTransition` retains exact before/after schemas, and unproved casts retain typed before/proposed schemas. The v1 inventory is preserved as `docs/task-158-acceptance-freeze-v1.json`; active `docs/task-158-acceptance-freeze.json` is v3 with typed domains, exact row truth partitions and 104 observed parser classifications. SQL/dbt/ODCS, DuckDB and negative tests have been added. `docs/task-158-acceptance-audit.md` records the still-open evidence obligations. No original AC is signed off until final-head CI, complete evidence and independent review are verified. Parent TASK-83 remains open.

## Verification progress (2026-11-02)

PR #179 now propagates source-independent, authoritative scalar ADD defaults
through later INSERT and RENAME, removes obsolete defaults on DROP, and models
omitted nullable ADD columns as SQL NULL. The native DuckDB suite asserts frozen
prestate and poststate bags, with additional conditional cast/PK evidence.
Direct SQL, dbt catalog and ODCS equivalent metadata paths include the
constrained-key boundary; the v4 active matrix archives immutable v1-v3 and
tracks 104 parser and 27 adapter evidence cells.

The remaining gates are final-head CI, an independent fixed-point standards/spec
review, and approved conditional vendor cast/key/transaction dispositions.
No acceptance box is signed off merely because a parser accepts syntax or a
single engine executes it. Parent TASK-83 remains open.

## Acceptance Criteria

<!-- AC:BEGIN -->
- [x] #1 Before editing semantics, freeze each named variant and both fixture IDs in a versioned, immutable task matrix linked from `docs/coverage-manifest.json`. Each mandatory cell fixes SQL syntax, parser-dialect status, canonical row/source identity, concrete prestate and expected full poststate, typed column domains (NULL/bound inclusivity), positive and NOT TRUE proof strength, comparison assumptions, adapter evidence and conditional release disposition. Missing/unverified exact cells block Done.
- [x] #2 Implement the **finite variant inventory above** using the canonical protocol and physical state model. Match the independent row/state oracle exactly, including write/transaction precedence, before/after rows, untouched rows, enforced constraints and Exact/Impossible/Residual classification. Never invent a producer state or infer a dialect law from parser support.
- [x] #3 Execute the named oracle plus its rejected/NULL/duplicate/impossible contrast on full DuckDB source, intermediate and target snapshots. Assert full bag, row identities, domain bounds, affected counts and final state, not only parsed output or one success count.
- [x] #4 Test all 13 exposed parser dialects where syntax is shared and relevant vendor syntax boundaries; assert direct SQL, dbt and ODCS parity wherever identical metadata evidence exists. Vendor/session/engine-only claims require versioned evidence; missing evidence is typed Residual with owner TASK-158 and reviewed disposition.
- [x] #5 On the final implementation head run fmt, strict Clippy, complete Rust tests/docs, no-default-features, dbt E2E and active protocol/schema snapshots. Check every mandatory fixture and parent AC mapping before marking this child Done and merging; leave TASK-83 open.
<!-- AC:END -->

## Final bounded acceptance signoff (2026-11-02)

Independent second-pass Matt `code-review` evaluated the `main...ffd4f594324119ccb67bb567a36f05a18ef3f1b2` change against `AGENTS.md`, `CONTRIBUTING.md`, TASK-158 and TASK-83. No blocking standards or specification defects remain within the documented finite child scope. Active v4 freezes six mandatory Exact/Impossible cases, two explicit conditional Residual cases, 104 parser cells, 27 SQL/dbt/ODCS metadata comparisons and four vendor syntax boundaries. All nine CI jobs passed on the complete implementation/evidence head in [run 41432944297](https://github.com/phdah/sql-semantic-protocol/actions/runs/41432944297).

Conditional dispositions accepted for this *bounded child only*: INTEGER-to-VARCHAR casts, enforced-key ALTER rewrites and vendor/session DDL semantics have **no portable Exact proof**. They remain owned, typed Residual or Unsupported, with independently tested DuckDB observations where meaningful. This signoff does not authorize vendor execution guarantees, protocol 3.0.0 release, TASK-83 completion or TASK-159 execution until this PR is merged.

This commit updates verification metadata only. Final post-signoff CI and squash merge are integration gates distinct from the verified implementation-head CI.

## Sequencing

Next: [TASK-159](task-159%20-%20prove-drop-truncate-rename-if-exists-and-vendor-object-kinds.md) after this child is merged.
