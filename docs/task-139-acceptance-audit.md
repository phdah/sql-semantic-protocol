# TASK-139 acceptance audit

Owner: TASK-139, TASK-77 child 3/4.  
Frozen fixture families: `77-C139-P`, `77-C139-N`.  
Inventory: [task-139-acceptance-freeze.json](task-139-acceptance-freeze.json) v3.  
Public proof seams: `analyze_inputs` / `analyze_dbt_manifest_with_schemas`,
`physical_finite_table_plan` and typed JSON emission.
This does not sign off TASK-77, TASK-91 or native vendor function execution.

## Exact core and typed physical evidence

The finite array literal `[5,6]` is an authoritative, source-free input.
The canonical `FiniteTableKind::Explode` stores output values, integer type,
stable positional producer identities and a complete terminal bag. The
public physical API computes finite value domains with inclusive extrema,
retains SQL NULL and duplicate multiplicity, accepts a present row with
Exact and returns Impossible for a fully typed absent tuple. It does not
invent a physical table for the literal expression.

Three mandatory executable cases are frozen and tested:

| Fixture | Canonical SQL | Expected terminal bag | IDs / typed domain |
| --- | --- | --- | --- |
| 77-C139-P-explode-select | `SELECT EXPLODE([5,6]) AS v` | `[5,6]` | `explode:1,explode:2`; integer [5,6] inclusive |
| 77-C139-N-explode-duplicates-null | `SELECT EXPLODE([5,NULL,5]) AS v` | `[5,NULL,5]` | `explode:1..3`; nullable integer 5 inclusive |
| 77-C139-P-explode-from | `SELECT * FROM EXPLODE([5,6]) AS t(v)` | `[5,6]` | `explode:1,explode:2`; integer [5,6] inclusive |

Every complete bag is checked against independent DuckDB UNNEST execution. The task-owned source oracle independently evaluates the literal array length and every element, including NULL slots, through DuckDB array functions; source-free relations still have zero *physical table* inputs.
DuckDB UNNEST demonstrates the array-element bag and NULL laws; it does
**not** certify native Spark EXPLODE, Snowflake FLATTEN, JSON_TABLE, or
dialect-specific ordering/coercion. Complete physical input relation bags
are intentionally empty because these cases are literal source-free
constructors; each generated output still has a stable producer-row ID.

## Conditional boundaries

The other four frozen variants cover JSON_TABLE dynamic JSON path, Snowflake
FLATTEN on PARSE_JSON, opaque generator/table UDF and variable schema.
Their unknown source cardinality, output shape, JSON path/session rules or
vendor runtime laws cannot be made Exact or Impossible from the parser alone.
All are reviewed as conditional Residual, with TASK-139 as owner and
unexecuted vendor/native runtime explicitly uncertified. They are not
reclassified mandatory cases and no static source/terminal population or
typed bounds are invented.

## Parser and adapter evidence

A CI execution of the frozen parser-classification discovery on
[run 40131023023](https://github.com/phdah/sql-semantic-protocol/actions/runs/40131023023)
established all 91 outcomes: **33 exact, 57 residual, 1 parser boundary**
across seven cases and 13 exposed dialects. The discovery probe deliberately
failed to reveal its results and was removed; `tests/task_139_frozen_matrix.rs`
now independently asserts those static expectations on every CI execution.
The matrix classifies parser capabilities only; a dialect-accepted SQL string
is not a native execution certification.

`tests/task_139_adapter_matrix.rs` checks 21 direct SQL, compiled dbt,
and ODCS-source dispositions. Real ODCS schema contracts describe
datatypes and constraints, not the literal array's complete element bag;
ODCS-only source evidence cannot certify positive/negative population
membership. `tests/task_139_dbt_core_e2e.rs` also checks two actual dbt
Core compiled and materialized DuckDB models of the equivalent UNNEST
array law and compares complete native bags and typed domain facts to
canonical EXPLODE. Operator identities are deliberately not conflated,
nor is native vendor EXPLODE execution claimed.

## Acceptance criterion mapping

| AC | Independent evidence and remaining gate |
| --- | --- |
| #1 | v2 frozen JSON, seven exact/conditional fixture variants, all 91 dialect statuses, 21 adapter dispositions, explicit identities, NULL/bounds and reviewed conditionals; final CI required |
| #2 | Canonical source-free FROM/SELECT EXPLODE; exact typed complete bags and impossible absent rows; vendor-specific semantics only reviewed Residual |
| #3 | `tests/task_139_frozen_matrix.rs` and `tests/task_139_generators.rs`: complete independent native DuckDB rows, duplicate/NULL identities, domains and both membership directions |
| #4 | `tests/task_139_frozen_matrix.rs`, `tests/task_139_adapter_matrix.rs`, `tests/task_139_dbt_core_e2e.rs`: 13 dialects, SQL/dbt/ODCS, real dbt Core and native DuckDB array oracles; non-DuckDB engines uncertified |
| #5 | Final-head `cargo fmt --check`, strict Clippy, tests, docs, no-default-features and dbt Core E2E must all pass before closure; inspect assertions and both Matt review axes |

## Independent standards/specification review

**Standards:** The change adds no dependencies, version exceptions or unsafe
code; AST pattern matching remains in `src/analysis/finite_tables.rs`,
canonical `FiniteTable` owns producer rows, and
`src/physical_realization/finite_tables.rs` proves membership without SQL
reparsing. Reuses existing finite source/domain types and preserves
deterministic output and the wire contract.

**Spec:** The exact literal-array `EXPLODE` core has complete positive,
NULL/duplicate and missing-row proofs, and unknown/dynamic vendor families
are conservatively classified with explicit reviewed evidence gaps. Final
parser, adapter and native E2E test results must be examined before signoff.
TASK-140 owns cross-operator fanout composition; TASK-139 does not imply
TASK-77 parent closure.

**Signoff status:** PENDING final-head CI and full review. Do not check TASK-139
acceptance criteria or mark Done solely from the discovery probe.
