# TASK-112 independent bounded acceptance audit

Parent: TASK-71 remains In Progress. PR: https://github.com/phdah/sql-semantic-protocol/pull/121. Dependency TASK-111 is Done and merged on main.

## Frozen SQL source-bag oracle

Versioned inventory: `docs/task-112-acceptance-freeze.json` v1. The eight mandatory cells cover LEFT/RIGHT SEMI and ANTI, duplicate keys, SQL UNKNOWN, and rejected nonmembership. Each cell binds stable source instances `l`/`r`, column instances `id`/`k`, exact SQL/AST expectations, 13 dialect parser classifications, source/terminal row-identity bags, inclusive numeric domain limits, NULL membership, and exact target multiplicities.

Physical SQL fixture P: L = `l1:1, l2:1, l3:NULL, l4:2`; R = `r1:1, r2:NULL`.

Physical SQL fixture N: same L; R = `r1:3, r2:NULL`.

| Frozen case | SQL result ID:key bag | Rows | Output key domain |
|---|---|---:|---|
| 71-C112-P-LEFT-SEMI | l1:1, l2:1 | 2 | [1,1], non-NULL |
| 71-C112-P-LEFT-ANTI | l3:NULL, l4:2 | 2 | [2,2] plus NULL |
| 71-C112-P-RIGHT-SEMI | r1:1 | 1 | [1,1], non-NULL |
| 71-C112-P-RIGHT-ANTI | r2:NULL | 1 | NULL only |
| 71-C112-N-LEFT-SEMI | empty | 0 | empty |
| 71-C112-N-LEFT-ANTI | l1:1, l2:1, l3:NULL, l4:2 | 4 | [1,2] plus NULL |
| 71-C112-N-RIGHT-SEMI | empty | 0 | empty |
| 71-C112-N-RIGHT-ANTI | r1:3, r2:NULL | 2 | [3,3] plus NULL |

Each bag is inserted into DuckDB with its exact physical identities and nullable keys, evaluated with native `SEMI`/`ANTI` on appropriately oriented inputs, and compared to the **full** sorted expected terminal bag. The test separately checks minimum/maximum inclusive bounds, NULL, duplicate multiplicity, and exhaustive physical partner membership for every preserved source row. Right-sided constructions are equivalently reoriented for independent DuckDB execution; this does not certify native `RIGHT SEMI/ANTI` grammar.

## Canonical physical witness, with sound NULL boundary

`src/constructive/types.rs` and `src/physical_realization/joins.rs` own checked `mixed_duplicate_left` and `disjoint_four_two` bag laws. They build one closed-world assignment per distinct physical source with 4 L and 2 R rows and explicit full-bag `NoMatchingPartner` obligations. SEMI counts source rows, not matching pairs: two duplicate matching left rows survive P-LEFT-SEMI exactly twice; P-RIGHT-SEMI selects one right row. ANTI excludes **every** TRUE partner, including when SQL equality on physical NULL evaluates UNKNOWN.

The **constructive** witness uses non-NULL source keys L=[0,0,1,1], R=[0,2] (mixed) or R=[2,2] (disjoint). `tests/task_112_semi_anti.rs` executes those entire source bags independently as NOT NULL DuckDB tables and verifies the **same survivor identities and multiplicities** as the nullable SQL fixture. It does **not** claim identical NULL value domains for these alternative concrete populations or physical SQL NULL insertability from `INTEGER` schema metadata. Literal SQL NULL behavior and output domains are certified separately by the immutable SQL oracle. The canonical obligations remain sufficient existential constructions, never necessary/exhaustive unique source assignments.

The protocol schema enumerates both new canonical patterns; `docs/coverage-manifest.json` links this audit's frozen inventory.

## Dialect, adapter, and conditional boundary evidence

- All 104 parser cells (8 SQL cases x 13 exposed canonical dialects) were observed parser-equivalent against the public SQL analyzer, with exact join-kind and equality operator checks. This is **parser/canonical equivalence**, not independent native vendor-engine certification.
- Direct SQL, compiled dbt manifest, and equivalent ODCS-supplied typed source metadata each produce closed physical SEMI/ANTI witnesses in `tests/task_112_semi_anti_adapters.rs` for LEFT/RIGHT SEMI and ANTI.
- `TASK-112-C-VENDOR`: native engine/session dialect laws remain unproven without vendor execution. Owner TASK-112, typed residual when needed, reviewed bounded conditional with **pending release review**, not silently certified.
- `TASK-112-C-NULLABILITY`: physical NULL insertion in a source with unknown nullable/NOT NULL constraints lacks authoritative source-side proof. The constructive alternative is non-NULL. Owner TASK-112, reviewed bounded conditional with **pending release review**. Literal SQL NULL is separately DuckDB-proven.

## Matt two-axis code review (against main)

**Standards:** Checked repository `AGENTS.md`, `CONTRIBUTING.md`, canonical ownership and Fowler smell baseline. No adapter-specific proof engines or consumer-side SQL reparsing. New types and output schema are versioned together, with public integration tests and conventional commits. Duplicate test-fixture setup is local to independently readable public tests rather than a new speculative helper abstraction. The initial physical-NULL-insertion overclaim was found and resolved by non-NULL constructive alternatives and a separate independent SQL NULL oracle.

**Spec:** All eight frozen mandatory join/direction/positive-and-rejected variants have concrete full DuckDB source/terminal bags, bound/NULL constraints, and canonical source-count witness assertions; four join kinds are checked for adapter parity. The original TASK-71 parent remains In Progress; TASK-113 remains blocked until this PR merges. Other dialect-native and NULL-insertability claims are not certified or misrepresented.

## Release and verification boundary

Nine CI jobs are required on the implementation head: formatting, strict Clippy, full Rust tests, docs, no-default-features lint/test/docs/dependencies, and dbt Core E2E. The final head also needs a green run after task and manifest signoff metadata. Task-112 only, not TASK-71 or 3.0.0, is eligible for bounded signoff. Pending release decisions are not automatically approved by a green check.
