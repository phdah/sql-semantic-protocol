# TASK-168 acceptance audit (draft, not signed off)

This bounded child implements correlated terminal histograms without claiming independently realizable source populations. The [preimplementation v1 acceptance freeze](task-168-acceptance-freeze.json) contains seven cases (six mandatory, one conditional), their exact physical sources and terminal requests, and 13 parser identities per case. Every mandatory cell must be reconciled before this child may be marked Done; original TASK-85 parent criteria remain separate.

## Implementation boundaries

- `src/physical_realization/joint_tuple_goals.rs::physical_joint_tuple_histogram_plan` verifies full correlated INTEGER/NULL tuple frequencies against **the same** closed physical JOIN/GROUP and WINDOW/SET source/terminal bags. It never multiplies marginal histograms. Contradictions against exact full bags are `Impossible`, while unverified typed scalar laws and absent output identities are `Residual`.
- `physical_single_tuple_histogram_plan` reuses the canonical `physical_finite_page_bag_plan`, `physical_finite_window_bag_plan` and `physical_finite_rank_bag_plan`, retaining complete source/terminal identities, rejected rows and exact typed domains for the supported ORDER/LIMIT and QUALIFY subset. Unsupported sort/session/tie semantics stay `Residual`.
- `tests/task_167_joint_terminals.rs` tests a correlated wrong-pair histogram with **identical per-column marginals** across the 13 portable parser identities, and complete SET duplicate/NULL bags. `tests/task_168_tuple_fanout.rs` asserts page/QUALIFY source, selected/rejected output, exact tuples, contradiction and independent DuckDB results.
- Existing canonical SQL output, parser model and protocol root are unchanged. No new top-level v3 schema or consumer contract is implied by these library-only typed proof entry points.

## Open signoff obligations

| Requirement | Evidence currently present | Still required |
|---|---|---|
| AC #1 frozen matrix | v1 frozen before code, linked by coverage manifest; seven named cases | Reconcile every mandatory cell to versioned executable output/domain/adapter and explicit 13-dialect parser status |
| AC #2 semantics | Source-attested tuple equality and full-bag contradiction; LIMIT and QUALIFY interpreter reuse | Full upstream source tuple-correlation/fanout proof for every frozen case; no ad hoc residual promotion |
| AC #3 native oracle | Full DuckDB source and tuple output assertions for window, set, page and QUALIFY | Frozen row identities and exact terminal domains for each individual case, including all rejected/Impossible counterparts |
| AC #4 adapters/dialects | Shared LIMIT and window/set parser examples | dbt + ODCS equivalent metadata parity and explicit classification for all 91 parser cells; vendor/session review |
| AC #5 final-head signoff | PR CI pipeline triggered | Complete fmt/strict Clippy/full/default/no-default/doc/dbt/golden + pinned sql-tdg consumer suite and independent reviewer approval |

**Status:** In Progress, no acceptance criteria checked, not merge-ready. `TASK-85` parent and `TASK-91` release gate remain open. A green ordinary CI run is necessary but not sufficient for signoff. The previous TASK-167 owner waiver does not transfer to this child.
