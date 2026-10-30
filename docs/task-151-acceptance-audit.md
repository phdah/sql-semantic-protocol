# TASK-151 bounded acceptance audit

**Child:** TASK-151, 3/4 of TASK-81. **Status:** Done in PR #163, pending merge. Parent TASK-81 remains independently unsigned.

## Spec review

The immutable [v3 inventory](task-151-acceptance-freeze.json), with historical v1/v2 retained, has **5 mandatory Exact, 2 mandatory negative, and 2 reviewed conditional vendor** cases, mapped to all five child ACs and parent AC #1..#6. Exact cases verify: qualified UPDATE/DELETE RETURNING, the specified two-key UPDATE returning `[1,2]`, duplicates, NULL/nonmembers, and one-source correlated EXISTS/NOT EXISTS over complete source/target bags. Every exact case fixes pre/post row identities, affected ordinals, typed NULL frequencies, lower/upper numeric bounds and inclusivity. An enforced-key collision is Impossible with independent DuckDB rejection; missing source stays Residual, never empty by assumption.

Vendor DELETE ORDER/LIMIT and partition-qualified UPDATE remain **explicitly Unsupported, typed Residual or parser boundary**, with TASK-151 responsible for native engine/version laws and partition metadata. These are reviewed *conditional* cells, not omitted mandatory exact proofs. The alias-as-PARTITION parser trap was fixed so it cannot claim unpartitioned whole-table mutation.

All **104** frozen parser classifications match 13 named dialects x 8 distinct SQL shapes: **52 canonical**, **42 unsupported**, **10 parser boundary**. Direct SQL, dbt catalog and ODCS equivalent schema evidence agree over 24 adapter-case paths; parser support is never treated as evidence of native vendor execution.

## Independent physical oracles

`tests/task_151_qualified_returning.rs` verifies full DuckDB target AND source snapshots, unchanged source, before/after bags, RETURNING postimages/preimages, duplicate/NULL frequencies, selected/rejected row ordinals, interval endpoints, missing source and enforced-key Impossible. `tests/task_151_dialect_adapter.rs` covers the dialect and adapter matrix. Active protocol JSON/schema snapshots preserve the existing wire shape.

## Matt code-review: Standards axis

A separate postimplementation Standards pass reviewed the diff against AGENTS.md, CONTRIBUTING.md, the canonical code map and Fowler smell baseline. The AST is confined to `src/analysis/writes.rs`, public normalized predicates retain exact source references, and physical evaluation uses the same complete bag in `src/physical_realization/write_states/operators.rs`. No new dependency, lint suppression, dialect-name dispatch, speculative output or public JSON schema change. Corrections during review included RETURNING alias shadowing, PARTITION ambiguity, and full native source/domain oracles. No further actionable defect found.

## Matt code-review: Spec axis

All finite mandatory cases, negative contrasts and adapter-parity cells are verified; vendor-only unknowns retain owned conditional boundaries. No parent TASK-81 signoff or TASK-152 ordered-script certification is implied. All nine jobs of [implementation CI run 41022690530](https://github.com/phdah/sql-semantic-protocol/actions/runs/41022690530) passed at `e7df77c338b3cf25c85ce78e1d882bce9d206b73`: fmt, strict Clippy, default/no-default Rust tests/docs/dependencies and dbt Core E2E.

## Merge gate

TASK-151 is Done on the PR branch, pending merge to main. The final status-only commit must pass its own CI. TASK-152 remains blocked until this completed state is merged.
