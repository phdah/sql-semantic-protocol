# TASK-153 acceptance audit

## Frozen contract
The original preimplementation eight-case matrix is preserved as docs/task-153-acceptance-freeze-v1.json. Version 2 preserves the observed 65 parser and 24 adapter cells. Active version 3 adds a separately bounded three-step ordered write replay to close the original child AC #2 precedence obligation; docs/coverage-manifest.json selects the active version. The stable fixture roots remain 82-C153-P and 82-C153-N.

## Acceptance crosswalk
- AC #1 (TASK-82 AC #1, #2, #3): tests/task_153_acceptance_inventory.rs enforces immutable fixture IDs, mandatory Exact/Impossible/Residual classification, expected typed source/target domains, NULL and NOT TRUE evidence, parser/adapter cells, parent mapping and manifest consistency.
- AC #2 (TASK-82 AC #1, #2, #3): canonical src/physical_realization/write_states/operators.rs proves first-applicable matched UPDATE/DELETE and exposes selected target prestate ordinals without guessing source matches. The ordered INSERT/MERGE/MERGE replay preserves sequential read and write preimages. Duplicate source matches remain Residual; enforced PK collision is Impossible.
- AC #3 (TASK-82 AC #3, #5): tests/task_153_merge_matched.rs compares full native DuckDB and canonical source/target bags, immutable source snapshots, zero independently created intermediate bags, affected ordinals/counts, typed numeric bounds and NULL frequencies, and atomic failure.
- AC #4 (TASK-82 AC #1, #2, #6): tests/task_153_dialect_adapter.rs checks 39 parsed canonical shared-syntax cells and 26 explicit vendor parser boundaries for 13 dialects; all eight frozen single-statement oracles compare direct SQL, dbt catalog and ODCS schema equivalent evidence. Parser success does not attest a native vendor law.
- AC #5 (TASK-82 AC #5, #6): final head must pass the complete nine-job CI matrix: format, strict Clippy, tests and docs with/without default features, dependencies and dbt Core E2E; active schema snapshots are covered by the full Rust test run. Final CI reference must be pinned after all metadata changes.

The frozen v3 contract owns five mandatory single-statement Exact cases, one Impossible key collision, two Residual cases and one mandatory ordered Exact replay. Every conditional vendor syntax case names TASK-153, is classified at the parser boundary in all 13 paths and has an explicit do-not-certify-without-native-evidence disposition.

## Standards and spec review
Standards: existing canonical module ownership maintained; no dependency, unsafe code, lint suppression, or hardcoded production dialect whitelist. A stale MERGE row-ordinal API comment was corrected, and unsupported MERGE OUTPUT now fails closed rather than silently advertising an exact write.
Spec: all five original child AC and parent AC #1, #2, #3, #5, #6 have a task-owned evidence path. Unmatched branches, repeated source match equivalence, vendor engine laws and computed sequential MERGE cases stay with TASK-154..156, not falsely signed off here.

## Independent review and merge gate
The audit is authored with the implementation and is not itself an independent reviewer signoff. TASK-153 stays In Progress until final-head CI and independent review are verified; TASK-82 remains open until all four children and original parent criteria are independently signed off.
