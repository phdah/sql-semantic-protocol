# TASK-145 independent acceptance audit

**Scope:** TASK-80 child 1/4; `80-C145-P` and `80-C145-N`.
Parent TASK-80 AC #1, #2, #3, #5, #6 are mapped to this child *only for
the finite source-free INSERT inventory*. Parent AC #4 and the remainder
of other criteria belong to TASK-146..148 and stay open.

## Frozen contract and correction

`docs/task-145-acceptance-freeze.json` v2 retains the twelve previously
frozen SQL forms and IDs. Its reviewed correction adds missing closed
score bounds and NULL/inclusivity evidence, and no longer represents an
unproved conditional result as a known empty poststate. The Git history
retains v1. Both source-free fixture families are versioned, with a
nonmembership witness distinct from all accepted rows. No named mandatory
case is reclassified as residual.

## Verification by acceptance criterion

| Child AC | Independent gate | Evidence |
| --- | --- | --- |
| #1 | 12 fixed cases, typed complete bags, two frozen IDs, bounds, 156 parser expectations, 36 adapter expectations, 7 reviewed vendor boundaries | `tests/task_145_acceptance_matrix.rs`; `tests/task_145_dialect_adapter.rs`; frozen v2 matrix |
| #2 | Exact physical multirow, implicit and explicit target bindings, one default-valued row, attested omitted/generated/sequence-generated fields; ordered before/after snapshots; constraint and transaction fail-closed boundaries | `tests/task_145_insert_values.rs`; `tests/task_145_typed_write.rs`; `tests/task_145_precedence.rs` |
| #3 | Full DuckDB source/target bags, NULL, duplicate, nonmembership, PK rejection, zero-row, touched/untouched rows, affected counts, values and inclusive bounds | `tests/task_145_acceptance_matrix.rs`; `tests/task_145_insert_values.rs`; `tests/task_145_precedence.rs` |
| #4 | All 12 cases * 13 parser dialects; complete equivalent target-schema proofs using direct SQL, dbt catalog and ODCS v3.2 metadata; seven vendor syntax residual/parser-boundary cells | `tests/task_145_dialect_adapter.rs`; `tests/task_145_vendor_boundaries.rs` |
| #5 | Required final-head CI and active schema/golden snapshots | GitHub Actions PR #155 CI, final-head signoff pending |

## Named case-by-case independent disposition

| Frozen case | Required proof | Independent review |
| --- | --- | --- |
| 80-C145-P-values-explicit | Exact | Entire [(1,10),(2,20)] bag, source-free finite rows and both inclusive bounds |
| 80-C145-P-implicit-order | Exact | Target declaration order, same complete two-row bag |
| 80-C145-P-reordered | Exact | Explicit score/id bindings preserve target id/score values |
| 80-C145-P-duplicate-bag | Exact | Two identical inserted rows remain independent physical instances |
| 80-C145-P-default-omission | Exact | Caller-attested score default 7 only, not inferred nullable NULL |
| 80-C145-P-default-values | Exact | One and only one row [(5,7)] from attested defaults |
| 80-C145-P-zero-source | Exact | Empty input leaves an exact unchanged empty target |
| 80-C145-N-null-value | Exact | Physical SQL NULL is not an untyped missing value |
| 80-C145-N-unique-violation | Impossible | Enforced id PK rejects whole INSERT without partial commit |
| 80-C145-N-unattested-default | Residual | No default/NULL fabrication |
| 80-C145-N-unattested-identity | Residual | No identity sequence allocation without authoritative metadata |
| 80-C145-N-cast-uncertainty | Residual | Unproved text-to-integer expression explicitly unsupported |

## Adapter, engine, and release boundaries

The Rust parser tests certify `sqlparser 0.58.0` AST and canonical
classification, **not native vendor SQL execution**. The only independently
executed engine is the locked DuckDB Rust dev dependency. Dbt catalog and
ODCS v3.2 tests verify equivalent target schema/physical-state evidence for
caller-owned INSERT statements. A dbt compiled model is **not** represented as
an independently executed DML write. These are proof-boundary facts, not missing
portable mandatory implementation.

Scalar and typed physical writes construct attested row-local generated fields through the same canonical evaluator. Typed writes without a caller-supplied sequence state remain Residual for identities. Identity allocation in the scalar planner is exact only with caller-attested sequence state.
Generated expressions are exact only for supported, attested row-local laws.
Opaque defaults, implicit casts, vendor ON CONFLICT/IGNORE/REPLACE/OVERWRITE/
RETURNING execution laws and real SQL transaction semantics are conditionally
uncertified: SQLparser acceptance alone cannot certify their row effects.
TASK-145 owns these typed residual safety boundaries; TASK-147 owns later
vendor conflict/overwrite/returning constructive semantics. No native
Snowflake, PostgreSQL, MySQL, or other vendor runtime is claimed.

**Reviewed code-head verification:** All nine Rust formatting, strict lint, complete
Rust test, docs, no-default-features and real dbt Core E2E jobs passed on
[implementation head aca4629](https://github.com/phdah/sql-semantic-protocol/actions/runs/40571172433).
All 12 frozen cases, 156 dialect parser cells, 36 parity cells and seven
vendor boundaries were independently reviewed against the above oracles.
The metadata-only signoff commit requires its own final-head CI before merge.
Keep parent TASK-80 To Do; TASK-146 may start only after this PR merges.
