# TASK-118 independent acceptance audit

## Frozen scope

- Parent TASK-72 stays open. Child TASK-118, bounded to nested three-branch portable set bags, branch-local WHERE, shared physical sources, and transparent named CTE/model producers.
- The frozen matrix [task-118-acceptance-freeze.json](task-118-acceptance-freeze.json) covers five complete cases, including positive `72-C118-P` and zero/nonmembership `72-C118-N` obligations. `docs/coverage-manifest.json` registers the evidence.
- The positive nested baseline `(A UNION ALL B) EXCEPT ALL C` uses A=2, B=1, C=1 copies of `(1,NULL)`, and yields two copies. The negative counterpart uses A=1, B=1, C=2 and yields an empty bag. Shared-branch variants retain physical row identities, SQL UNKNOWN witnesses and typed terminal bounds.

## Public implementation and independent evidence

- `src/analysis/sets.rs` certifies only single-source identity-preserving local CTEs; filtered, limited, sampled or shaped CTE producers do not become writable physical inputs. `src/protocol/sets.rs` retains canonical nested operator law and leaf shape.
- `src/physical_realization/set_trees.rs` interprets canonical nested DISTINCT/ALL multiplicity on *one complete population per physical relation*, with per-branch TRUE/FALSE/UNKNOWN row identity partitions, full nullable tuple histograms, terminal domains, Exact/Impossible/Residual results and no SQL reparsing. Transparent named models are traced to their original physical source; unattested type transport remains Residual.
- `tests/task_118_nested_set_bags.rs` asserts all five independent DuckDB full-terminal/source-bag oracles, entire public terminal tuples, duplicate/NULL counts, stable physical source identities, typed inclusive bounds, negative absence, schema-enforced impossibility, and residual filtered/limited CTE boundaries.
- `tests/task_118_dialect_matrix.rs` asserts 65 canonical parser-law cells (five cases times thirteen project dialect identities).
- `tests/task_118_set_adapters.rs` asserts 15 SQL/dbt/ODCS equivalent physical source proof cells (five cases times three adapters). The materialized model case uses a paired dbt manifest **and catalog** to attest intermediate type transport; manifest-only model types are not silently promoted to warehouse evidence.

## Standards review

- New public API is additive and documented; canonical protocol remains consumer-independent, deterministic, and no new dependencies or `unsafe` were introduced.
- Shared source branches cannot independently create or mutate intermediate populations. Missing source rows, duplicate physical row IDs, unsupported predicates and missing type laws fail closed; enforced incompatible schemas are Impossible.
- Public SQL/analysis, dbt/ODCS artifact and complete DuckDB execution oracles are separate test seams. No result depends on implementation-generated snapshots alone.

## Specification review and release boundaries

- All five mandatory frozen bags, the three portable composition mechanisms, SQL three-valued filtering, NULL-safe *set tuple identity*, nested duplicate frequencies, zero/nonmembership and domain bounds have explicit tests.
- Original TASK-72 requirements mapped to TASK-118 are fulfilled only for this bounded portable slice. TASK-72 remains open for TASK-119 and TASK-120; no parent or vendor-wide completion is claimed.
- Thirteen parser identities are not thirteen independently executed database engines. Native vendor coercion/collation/session semantics, arbitrary producers, unsupported predicate forms, and set-level LIMIT/FETCH remain uncertified. These are conditional, fail-closed boundaries owned by TASK-118, not silent `Exact` proofs.

## Verification and closure

- [CI run 39228337486](https://github.com/phdah/sql-semantic-protocol/actions/runs/39228337486) passed all nine jobs on implementation commit `f3b319ee`: format, strict lint, Rust tests, no-default-features lint/tests/dependencies, docs and dbt Core E2E. It includes all five cases, the produced-model paired catalog oracle, 65 parser law cells and 15 adapter parity cells.
- Independent review axes: **Standards: 0 unresolved findings. Spec: 0 unresolved findings for bounded TASK-118.** No native engine-runtime release attestation is inferred.
- Final-head CI after this documentation/coverage status change must pass before merge. If a check fails, reopen TASK-118 rather than relying on this audit.
