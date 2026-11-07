# TASK-175 bounded acceptance audit (2026-11-07)

**Owner:** TASK-175, child 3/4 of TASK-87. **PR:** https://github.com/phdah/sql-semantic-protocol/pull/194. TASK-87 and TASK-91 are separate gates and remain open.

## Scope and frozen evidence

The eight original cases and fixtures `87-C175-P` / `87-C175-N` were recorded before semantic implementation in `docs/task-175-acceptance-freeze-v1.json` (six mandatory, two conditional). Active v2 adds 78 named mandatory parser cells (six cases × 13 exposed parser dialects), 19 adapter cells, exact DuckDB source/terminal snapshots and test ownership. V2 retains all v1 case definitions, physical populations, classifications and fixture IDs. **Historical process deviation:** v1 did not independently enumerate every dialect/adapter cell and was incomplete under the strict preimplementation freeze policy. V2 adds this evidence after implementation. This is disclosed rather than misrepresented as pre-frozen.

## AC-by-AC technical verification

| Child AC | Technical evidence and disposition |
| --- | --- |
| #1 Freeze and coverage | Immutable original v1, additive v2 cells and `docs/coverage-manifest.json`. All mandatory cases carry typed ranges, inclusive bound, NULL/duplicate source bags, FALSE/UNKNOWN negatives, source identities, adapter owner, vendor boundary and exact independent DuckDB terminal results. Procedural timing exception disclosed. |
| #2 Canonical implementation | `src/dbt.rs` checks `to` independently of `depends_on`. Missing/ambiguous declared and test targets retain relation-scoped diagnostics and no fabricated FK, while unrelated valid key facts survive. FK-only source schemas are present using catalog authority or complete manifest declaration; enforcement remains unknown. `src/relation.rs` owns qualified, quoted and dialect-normalized resolution; no duplicate resolver introduced. |
| #3 Physical and negative oracles | `tests/task_175_adapter_boundaries.rs` executes each of the eight frozen cases over its own DuckDB source and complete terminal rows, verifying two duplicates, NULL, FALSE, UNKNOWN and positive paths, plus canonical typed physical-READ proof with exact source ordinals and outcome frequencies. Missing FK-only schema explicitly errors. |
| #4 Dialects/adapters | 78 named parser cells check the portable WHERE filter and interval domain across 13 parser dialects. Case-specific quoted identities are tested on PostgreSQL/DuckDB representations; other vendor syntax and runtime guarantees are **not** inferred from portable parser acceptance. 19 adapter evidence cells include direct DDL, dbt manifest, dbt catalog and ODCS. Paired SQL/dbt/ODCS key/schema parity, dbt-source aliases and ODCS ambiguity are independently asserted. |
| #5 Complete integration gate | Final CI includes rustfmt, Clippy `-D warnings` for both feature sets, full Rust tests/docs, dependency checks, dbt E2E, active schema/goldens and DuckDB differential tests. The pinned `phdah/sql-tdg` crate uses published `sql-semantic-protocol = { version = "2.0.3", default-features = false }`; this patch changes only the dbt adapter diagnostics, no consumed protocol wire type or direct-SQL physical path. Running its pinned tests against 2.0.3 would not validate this unreleased PR. The downstream v3 consumer pin/upgrade remains its separate TASK-91 release gate, **not** a fictitious executed test here. |

## Frozen cases and dispositions

- `87-C175-P-quoted-qualified`: named, quote-preserving source identity with exact typed terminal proof.
- `87-C175-P-fk-only`: catalog-authoritative typed FK-only source and resolvable relationship; database enforcement unproved, not promoted.
- `87-C175-P-tests`: dbt built-in-test constraint provenance and unknown enforcement, no test execution inferred.
- `87-C175-P-canonical-parity`: one canonical key/schema/typed-domain fact across SQL, dbt and ODCS with separate provenance.
- `87-C175-N-missing-relationship-target`: explicit `unresolved_relationship_target` or `ambiguous_relationship_target` with no emitted FK.
- `87-C175-N-missing-declared-fk-target`: explicit `unresolved_foreign_key_target`; other valid constraints retained.
- `87-C175-N-untyped-fk-source` (conditional, owner TASK-175): explicit `MissingConstraintSchema` or `MissingDeclaredColumnTypes`; a real catalog/declared type is required before exactness. No warehouse introspection is fabricated.
- `87-C175-N-ambiguous-quoted-alias` (conditional, owner TASK-175): fail-closed ambiguous relation error with ordered candidate identities across direct SQL/dbt/ODCS where expressible; explicit qualified source is required to remove uncertainty.

The last two are reviewed **conditional boundaries**, not a waiver for missing mandatory semantics. Native engine validation is confined to DuckDB. No vendor/session or external consumer execution is claimed.

## Two-axis review and signoff boundaries

**Standards axis:** Code remains in the existing dbt/relationship owner and canonical relation resolver; no new dependencies, SQL reparsing in consumer, new wire schema variants, unsafe blocks, ignored lint or public API break. Existing adapter constraint diagnostic emission and typed source schema APIs are reused. CI's strict formatting/lint/documentation checks are authoritative.

**Spec axis:** Exact positive and NOT TRUE physical bags, NULL/duplicates, quoted/ambiguous relation paths, FK-only metadata, missing-target fail-closed behavior, adapter/13-dialect matrix and explicit non-native boundaries are covered. Existing whole-parent TASK-87 and unreleased TASK-91 gates are preserved.

**Review-process exception:** A two-axis review submitted through the connected PR author's account is **not** independent second-human approval. TASK-175 technical owner-directed signoff can record this exception transparently, but does not satisfy any separate GitHub branch protection or organizational peer-review requirement. The v1-to-v2 freeze timing issue is also not retroactively repaired.
