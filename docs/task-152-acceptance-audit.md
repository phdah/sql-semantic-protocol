# TASK-152 bounded acceptance audit

**Child:** TASK-152, 4/4 of TASK-81. **Status:** implementation under verification in PR #164. Parent TASK-81 remains independently unsigned.

## Frozen specification and test seams

The versioned [TASK-152 frozen cases](task-152-acceptance-freeze.json)
with retained [v1](task-152-acceptance-freeze-v1.json), [v2](task-152-acceptance-freeze-v2.json), and [v3](task-152-acceptance-freeze-v3.json) fix stable
`81-C152-P` and `81-C152-N` identities, complete target and parent
prestates, every committed intermediate state, distinct row occurrence
identity, typed numeric NULL/frequency/bounds, affected counts and
TRUE/IS NOT TRUE partitions. The independent numerical baseline is
empty target → INSERT (1,7,10),(2,7,20) → UPDATE id=1 score+5 →
DELETE id=2 → one row (1,7,15). Contrast cases cover nullable foreign
keys and untouched rows; NULL predicates are UNKNOWN; enforced CHECK
and primary-key violations are Impossible.

The v1 inventory incorrectly assumed a missing referenced FK tuple
was universally Impossible. The canonical interpreter intentionally
returns **Residual** when deferral/enforcement timing is not attested.
The reviewed v2 freeze corrects this exactness claim and records the
relevant conditional TASK-152/TASK-84/TASK-88/TASK-91 release owner.
Independent native DuckDB *does* reject that insert under its immediate
constraint semantics, but the portable protocol does not inherit the
engine-specific law. Omitting the entire referenced parent population
is a distinct `MissingPopulation` Residual, never an empty table.

## Public and native acceptance seam

`tests/task_152_ordered_mutations.rs` obtains the canonical protocol
from public configured inputs and attested schema, and calls the public
ordered physical-write interpreter. Its comparisons use literal
expected full-bag rows from the frozen matrix, with per-transition
counts, identities, nullable typed domains, and complete native DuckDB
target/parent bags before and after each successful or rejected write.
A failed SQL statement must preserve its previous complete target bag;
only previous committed statements persist in the oracle. No
transactional rollback across multiple statements is inferred.

`tests/task_152_dialect_adapter.rs` freezes 39 parser classifications
(13 exposed dialects x 3 common DML statements) plus 26 explicit vendor boundaries (13 x 2). The vendor boundary inventory reuses the executable TASK-151 dialect oracle indices: CASES[4] for ordered DELETE and CASES[5] for partition-qualified UPDATE. The v3 index mismatch was corrected in v4, retaining previous freezes as review history. All these vendor shapes must remain parser-boundary or canonical-unsupported, never portable Exact. For equivalent
schema metadata, independent direct SQL, compiled dbt catalog and ODCS
paths must yield identical physical-source and terminal-state results
under the same explicitly attested PK/FK/CHECK facts. Adapter
metadata alone does not confer enforced constraints or vendor-native
execution certification.

## Matt code-review: Standards axis

Reviewed against `AGENTS.md`, `CONTRIBUTING.md` and the canonical
code map. Changes stay at public integration seams and task-owned
evidence files; the canonical Rust semantics, protocol wire and JSON
schema are unchanged. No consumer-side SQL reparsing, silent
downgrades of mandatory cases, invented source rows or unverified
vendor claims. Earlier compiler and formatting defects were resolved
following the first CI run.

## Matt code-review: Spec axis

The task-owned positive, negative and conditional inventories map to
child AC #1..#5 and original TASK-81 AC #1..#6. The main requirement
is **ordered state provenance**, not three individually passing SQL
statements: every read and mutation must see the previous committed
target image. Enforced CHECK/PK contradictions and canonical FK deferral
Residual remain distinct. The adapter and native evidence is separately
tested so a parser result cannot stand in for a physical bag proof.

## Signoff and merge gate

Final PR-head `cargo fmt --check`, strict Clippy, default/no-default
Rust tests/docs, dbt E2E and active protocol/schema snapshots **must
all be successful before** changing TASK-152 to Done or checking its
acceptance boxes. Green CI alone is insufficient; review every frozen
case against actual native source and terminal bags. TASK-81 remains
open pending independent original-AC signoff and TASK-91 remains a
separate publication gate.
