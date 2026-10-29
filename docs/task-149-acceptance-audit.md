# TASK-149 acceptance audit

**Scope:** bounded TASK-81 child 1/4. TASK-81 remains open, as do the dependent TASK-150..152. This audit does not approve protocol v3 or native vendor equivalence.

**Specification:** `.backlog/tasks/task-149 - prove-typed-update-and-delete-single-target-effects.md`, original AC #1..#5. Frozen v2 matrix: `docs/task-149-acceptance-freeze.json`; v1 history retained in `docs/task-149-acceptance-freeze-v1.json`. The review added missing `NOT TRUE` source identities, exact column frequencies, simultaneous assignments, empty RETURNING, and enforced-key contradictions without downgrading mandatory cells.

## Inventory and oracle ownership

- **13 frozen cases:** 11 mandatory complete exact physical outcomes, one enforced-primary-key `Impossible`, one missing-authority `Residual`; fixture IDs `81-C149-P` and `81-C149-N`.
- **169 explicitly observed parser cells:** 13 cases × all 13 named dialects. Each parsed to canonical UPDATE/DELETE. Parser classification **does not** establish that a native vendor executes the form. CI discovery: [run 40870780989](https://github.com/phdah/sql-semantic-protocol/actions/runs/40870780989); future checks compare the frozen per-cell value.
- **39 adapter cells:** for every one of 13 cases, complete physical outcomes or the same typed impossibility/residual are compared using equivalent direct SQL, dbt manifest/catalog metadata, and ODCS evidence. CI proof: [run 40871254971](https://github.com/phdah/sql-semantic-protocol/actions/runs/40871254971), `tests/task_149_dialect_adapter.rs`.
- **Physical and native oracle:** `tests/task_149_frozen_matrix.rs` verifies each attested full target prestate and final target bag, all SQL TRUE vs IS NOT TRUE target row ordinals (including duplicate and NULL rows), affected counts, exact nullable integer value domains and their per-value frequencies, UPDATE postimages, DELETE preimages, and native DuckDB RETURNING. No invented intermediate source is accepted. `tests/task_149_update_delete.rs` exercises the wire contract, simultaneous SET semantics, atomic enforced PK error, absent default, empty RETURNING and zero-selected-row invalid target preflight.
- **Vendor boundary:** `tests/task_149_dialect_adapter.rs` executes 65 vendor syntax/dialect classifications and requires every parsed but unsupported joined, ORDER/LIMIT or RETURNING projection form to remain `Unsupported`. Other cross-table mutations and transaction semantics belong to TASK-150/151 and TASK-84, respectively.

## Original acceptance criterion mapping

| Child AC | Mandatory evidence | Disposition |
| --- | --- | --- |
| #1 Frozen per-cell inventory | v2 immutable SQL/row/domain/truth/dialect/adapter fixture contract; archived v1 | Complete, subject to final CI |
| #2 Physical semantics | `src/{analysis,protocol,physical_realization}/...` mutation owner; same physical before/after bag; constraint, NULL and missing metadata proofs | Complete, subject to final CI |
| #3 Independent execution | `tests/task_149_{frozen_matrix,update_delete}.rs`, 11 complete outcomes and positive/rejected/nonmembership/impossible contrasts | Complete, subject to final CI |
| #4 Dialects and adapters | 169 pinned parser statuses; 39 direct/dbt/ODCS parity cells; 65 vendor-boundary classifications | Complete, subject to final CI |
| #5 Final-head verification | fmt, strict Clippy, full and no-default-features Rust tests and docs, dbt Core E2E, active schema/golden snapshots | **Pending final green CI head; record link before checking this criterion** |

## Conditional boundaries reviewed

**TASK-149 owner:** no authoritative column DEFAULT metadata is a typed `Residual`, not SQL NULL or Exact. Unknown or invalid target columns and unproved computed expressions fail closed, including zero-row mutations. Native-vendor trigger, RETURNING, transaction/error ordering, nonportable ORDER/LIMIT and joined UPDATE/DELETE laws are not claimed from parser compatibility. The only independently executed SQL engine here is DuckDB. Native extensions require versioned engine/session evidence and remain under separately owned TASK-150/151/84/88 gates.

## Merge and parent gates

The child must not be marked `Done` until the complete final-head CI run is green and the PR is merged to `main`. TASK-81 original AC remain unchecked until TASK-149..152 land and an independent parent signoff is performed. TASK-91 release approval is entirely separate.
