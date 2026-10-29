# TASK-149 acceptance audit

**Scope:** bounded TASK-81 child 1/4. TASK-81 remains open, as do the dependent TASK-150..152. This audit does not approve protocol v3 or native vendor equivalence.

**Specification:** `.backlog/tasks/task-149 - prove-typed-update-and-delete-single-target-effects.md`, original AC #1..#5. Frozen v2 matrix: `docs/task-149-acceptance-freeze.json`; v1 history retained in `docs/task-149-acceptance-freeze-v1.json`. The review added missing `NOT TRUE` source identities, exact column frequencies, simultaneous assignments, empty RETURNING, and enforced-key contradictions without downgrading mandatory cells.

## Inventory and oracle ownership

- **13 frozen cases:** 11 mandatory complete exact physical outcomes, one enforced-primary-key `Impossible`, one missing-authority `Residual`; fixture IDs `81-C149-P` and `81-C149-N`.
- **169 explicitly observed parser cells:** 13 cases × all 13 named dialects. Each parsed to canonical UPDATE/DELETE. Parser classification **does not** establish that a native vendor executes the form. CI discovery: [run 40870780989](https://github.com/phdah/sql-semantic-protocol/actions/runs/40870780989); future checks compare the frozen per-cell value.
- **39 adapter cells:** for every one of 13 cases, complete physical outcomes or the same typed impossibility/residual are compared using equivalent direct SQL, dbt manifest/catalog metadata, and ODCS evidence. CI proof: [run 40871254971](https://github.com/phdah/sql-semantic-protocol/actions/runs/40871254971), `tests/task_149_dialect_adapter.rs`.
- **Physical and native oracle:** `tests/task_149_frozen_matrix.rs` verifies each attested full target prestate and final target bag, all SQL TRUE vs IS NOT TRUE target row ordinals (including duplicate and NULL rows), affected counts and `PhysicalWriteTransition::affected_target_ordinals()` complete prestate row-identity evidence, exact nullable integer value domains and their per-value frequencies, UPDATE postimages, DELETE preimages, and native DuckDB RETURNING. No invented intermediate source is accepted. `tests/task_149_update_delete.rs` exercises the wire contract, simultaneous SET semantics, atomic enforced PK error, absent default, empty RETURNING and zero-selected-row invalid target preflight.
- **Vendor boundary:** `tests/task_149_dialect_adapter.rs` executes 65 vendor syntax/dialect classifications and requires every parsed but unsupported joined, ORDER/LIMIT or RETURNING projection form to remain `Unsupported`. Other cross-table mutations and transaction semantics belong to TASK-150/151 and TASK-84, respectively.

## Original acceptance criterion mapping

| Child AC | Mandatory evidence | Disposition |
| --- | --- | --- |
| #1 Frozen per-cell inventory | v2 immutable SQL/row/domain/truth/dialect/adapter fixture contract; archived v1 | Verified; final CI evidence recorded |
| #2 Physical semantics | `src/{analysis,protocol,physical_realization}/...` mutation owner; same physical before/after bag; constraint, NULL and missing metadata proofs | Verified; final CI evidence recorded |
| #3 Independent execution | `tests/task_149_{frozen_matrix,update_delete}.rs`, 11 complete outcomes and positive/rejected/nonmembership/impossible contrasts | Verified; final CI evidence recorded |
| #4 Dialects and adapters | 169 pinned parser statuses; 39 direct/dbt/ODCS parity cells; 65 vendor-boundary classifications | Verified; final CI evidence recorded |
| #5 Final-head verification | fmt, strict Clippy, full and no-default-features Rust tests and docs, dbt Core E2E, active schema/golden snapshots | **Verified** on [CI run 40873925034](https://github.com/phdah/sql-semantic-protocol/actions/runs/40873925034) at implementation SHA `fd705b0b71fd645b811c4a32f9291b81baae1b99`: all nine jobs green. Public identity contract documentation is a subsequent metadata-only amendment. |

## Conditional boundaries reviewed

**TASK-149 owner:** no authoritative column DEFAULT metadata is a typed `Residual`, not SQL NULL or Exact. Unknown or invalid target columns and unproved computed expressions fail closed, including zero-row mutations. Native-vendor trigger, RETURNING, transaction/error ordering, nonportable ORDER/LIMIT and joined UPDATE/DELETE laws are not claimed from parser compatibility. The only independently executed SQL engine here is DuckDB. Native extensions require versioned engine/session evidence and remain under separately owned TASK-150/151/84/88 gates.

## Merge and parent gates

The child must not be marked `Done` until the complete final-head CI run is green and the PR is merged to `main`. TASK-81 original AC remain unchecked until TASK-149..152 land and an independent parent signoff is performed. TASK-91 release approval is entirely separate.

## Independent code review of PR #161 against `main`

**Standards:** the SQL parser is restricted to the analysis owner; protocol assignments and physical outputs have typed canonical ownership; public serialization updates include the schema and docs, no engine-dialect whitelist was introduced, unknown inputs fail closed, and no unsafe or lint suppressions were added. The formatter, all-target strict Clippy, documentation, and no-default-features checks passed on the implementation head.

**Spec:** all five child criteria map to finite mandatory cells, including independently executed full target bags with SQL TRUE/NOT TRUE identities, enforced constraint impossibility, absent metadata residual, default/RETURNING wire shape, complete typed frequency domains, 13 dialect parser classifications, and direct/dbt/ODCS parity. Joined mutations, nonportable native laws and transaction proof were not silently promoted to exact. No outstanding mandatory TASK-149 family gap was identified in this bounded review.

**Verification on same implementation SHA:** [run 40873925034](https://github.com/phdah/sql-semantic-protocol/actions/runs/40873925034), nine green checks. The review/contract text and acceptance signoff documents are additive; any final merge must still satisfy the PR's own head checks. TASK-81 parent and release approval remain outside this signoff.
