# TASK-167 technical review and acceptance gates

Date: 2026-11-03. Parent TASK-85 stays open.

## Prospective freeze

- The pre-implementation v1 acceptance inventory is in `docs/task-167-acceptance-freeze.json` and linked by `docs/coverage-manifest.json`.
- Mandatory families: positive JOIN/WHERE/GROUP/HAVING + window, negative joint grouped and window goals, positive shared-source positional UNION ALL, and negative set histogram/count. Conditional family: unknown vendor/opaque terminal remains Residual.
- Stable fixture IDs are `85-C167-P` and `85-C167-N`.
- Canonical parser dialect inventory is 13 (the test helper includes `postgres` as an additional PostgreSQL alias, not a 14th supported dialect).
- Native evidence: DuckDB only. No native vendor-engine claims.

## Standards review

- The proof reuses the existing canonical `physical_finite_join_aggregate_plan`, `physical_finite_window_bag_plan`, `physical_finite_set_tree_plan`, and closed-world schema validation. It does not parse SQL inside the physical interpreter or conjure independent writable producer rows.
- It refuses unsupported or opaque operations, and reports contradictions only against the caller-attested complete source bags.
- Initial GitHub CI established compiled Rust lint and documentation passes; initial formatting discrepancies were corrected using its exact rustfmt patch. The first full test run identified a mismatch between the 13 canonical dialects and the 14-entry helper with the extra `postgres` alias; this was corrected. Final-head verification is outstanding.

## Spec review and remaining work

| AC | Current status | Remaining acceptance gate |
| --- | --- | --- |
| #1 | Freeze v1 created before implementation; manifest linked | Verify each mandatory and conditional cell independently; capture signed output bounds and source ordinals |
| #2 | Bounded joint interpretation implemented | Check complete source/terminal correlation and arbitrary alternate witnesses outside this snapshot-relative API; all unsupported families explicitly remain Residual |
| #3 | Public API and independent DuckDB snapshots added | Verify final-head CI plus exact protocol claims and all negative native replay cases |
| #4 | All canonical dialect cases included in frozen matrix; direct SQL/dbt/ODCS test added | Confirm CI for adapter parity and record per-dialect parser verdicts and vendor limitations |
| #5 | CI started, with initial failures addressed | Final-head full Rust, no-default, fmt, Clippy, docs, dbt E2E, pinned sql-tdg consumer and independent reviewer acceptance |

This draft is **not** an acceptance signoff. All five child criteria remain unchecked and no unverified cell is marked Done. Formal signoff requires a later independently reviewed revision; TASK-168 remains blocked until TASK-167 is Done and merged.

## Review disposition

Matt Pocock `engineering/implement` was used for this implementation, and `engineering/code-review` Standards/Spec axes were applied as a first-pass author review of the branch against `main`. This is not independent reviewer identity signoff. The outstanding spec findings are the same explicit gates above, rather than silently reclassifying unsupported proofs as Exact.
