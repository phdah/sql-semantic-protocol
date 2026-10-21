# TASK-127 acceptance audit (PR #138)

Status: **not signed off**. Keep TASK-127 In Progress, the PR draft, and parent TASK-74 open.

## Independent evidence and dispositions

| Frozen case | Canonical physical proof | Independent engine evidence | Review |
| --- | --- | --- | --- |
| 74-C127-P1 | Exact direct-source survivors and NOT TRUE identities | `tests/task_127_qualify_bags.rs` | Direct subset verified |
| 74-C127-P2 | Exact conjunctive rank membership | `tests/task_127_qualify_bags.rs` | Direct subset verified |
| 74-C127-P3 | Exact RANK/DENSE_RANK membership | `tests/task_127_qualify_bags.rs` | Direct subset verified |
| 74-C127-N1 | Exact zero survivors and complete rejection | `tests/task_127_qualify_bags.rs` | Direct subset verified |
| 74-C127-P6 | Exact peer-stable RANK/DENSE_RANK with multiplicities | `tests/task_127_qualify_bags.rs` | Direct subset verified |
| 74-C127-N2 | Residual underordered ROW_NUMBER peer identities | `tests/task_127_qualify_bags.rs` | Conditional, intentionally not Exact |
| 74-C127-P4 | **Missing** exact two-source JOIN-to-QUALIFY composition | `tests/task_127_upstream_native_oracles.rs` | Mandatory blocker |
| 74-C127-P5 | **Missing** exact grouped producer-to-QUALIFY composition | `tests/task_127_upstream_native_oracles.rs` | Mandatory blocker |

All eight frozen SQL cases were observed to parse with a canonical QUALIFY predicate on the thirteen exposed parser paths (104 cells), recorded in `docs/task-127-acceptance-freeze.json` and guarded by `tests/task_127_dialect_classifications.rs`. This is **not** evidence that every native SQL engine implements the same QUALIFY execution laws.

The five Exact direct-source cases and the conditional peer case have direct SQL, compiled dbt and ODCS schema-evidence regression tests in `tests/task_127_adapter_parity.rs`. Joined and grouped adapter-equivalence cases remain mandatory and pending.

## Original TASK-74 acceptance mapping and child AC audit

- **Child AC #1:** Freeze is registered and the parser matrix observed. Joined/grouped adapter and full physical identity cells are still unresolved. **Not signed off.**
- **Child AC #2:** Direct physical bag, typed value domains, membership and rejection are implemented; joined/grouped source producer composition is missing. **Not signed off.**
- **Child AC #3:** The direct subset has independent complete source and DuckDB terminal oracles; P4/P5 independent native oracles were added without elevating their missing canonical physical proofs. **Not signed off.**
- **Child AC #4:** 104 parser paths are classified. Six direct cases have three-source-evidence parity; P4/P5 do not. Parser success is not native runtime certification. **Not signed off.**
- **Child AC #5:** Re-run and review final-head formatting, strict Clippy, full tests, docs, no-default-features, dbt E2E, and schema/golden checks after mandatory semantics land. **Not signed off.**

Original TASK-74 **#1, #3, #4, #5, #6** remain owned by TASK-127 for its child contribution. This review cannot close the parent TASK-74's original AC.

## Required next implementation

1. Materialize JOIN P4 against **both** complete source bags from canonical join semantics, with physical pair identities, selected/rejected joined rows, and explicitly classified nonjoining Cartesian pairs. Preserve join multiplicity and source datatypes.
2. Materialize GROUP P5 using canonical aggregate semantics and complete contributor identities, then evaluate QUALIFY against exactly those grouped rows. Preserve group identities and aggregate output datatypes.
3. Extend the physical proof representation if needed: do not mislabel a joined pair or a derived group as one independently controlled physical source. Exact row identity and full domains must survive the composed operator graph.
4. Check P4/P5 parity through direct SQL, compiled dbt and ODCS, all 13 dialects where parsed, and complete DuckDB oracles; close the freeze, update the coverage manifest and task checkboxes only after verification.
5. Run the final CI gates on the **last** PR commit and review the complete mandatory and conditional matrix. Never substitute a Residual for P4/P5's mandatory Exact requirement.

No release or merge approval is recorded by this document.
