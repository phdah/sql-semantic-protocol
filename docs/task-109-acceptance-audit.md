# TASK-109 independent acceptance audit (2026-10-17)

**Decision: signed off for the finite TASK-109 child scope.** This is **not**
TASK-71 parent signoff and does **not** approve protocol 3.0.0 or sql-tdg.
The executable acceptance freeze is
[task-109-acceptance-freeze.json](task-109-acceptance-freeze.json), v2.
The original five child acceptance criteria have not been weakened.

## Verification baseline

- Independently verified implementation HEAD: `e4ce883e01b17d027ce63efa1e42a53e9dfc757f`.
- GitHub Actions: [run 39025591446](https://github.com/phdah/sql-semantic-protocol/actions/runs/39025591446), **9 of 9 successful**: fmt, strict all-feature Clippy, full tests, docs, no-default test/lint/doc/dependency gate, dbt Core E2E.
- Final PR HEAD CI must also pass **after** the task-status and sign-off commits. The GitHub PR checks, not this prose, are the authoritative final-head result.
- Public executable frozen coverage:
  `tests/task_109_frozen_oracles.rs`,
  `tests/task_109_composite_join.rs`, and
  `tests/task_109_acceptance_freeze.rs`.
  The active schema and other project regression suites also run through full CI.

## Frozen exact-mandatory evidence

| Variant ID | SQL case | Closed physical pairs | Positive terminal rows | SQL NOT TRUE input pairs |
| --- | --- | ---: | ---: | ---: |
| `71-C109-P-EQ` | one-key INNER | 9 | 4 | 5 |
| `71-C109-P-COMPOSITE` | two-key INNER | 9 | 1 | 8 |
| `71-C109-N-NULL` | equality JOIN, null-rejected WHERE | 9 | 0 | 9 |
| `71-C109-N-COMPOSITE` | two-key JOIN, additional ON restriction | 9 | 0 | 9 |
| `71-C109-P-DUP` | 2-by-3 repeated-key bag | 6 | 6 | 0 |

Every row executes its **actual frozen SQL** against independent DuckDB
tables populated from the full frozen physical bags, checking the **entire
output value bag**, counts, NULL membership, inclusive lower/upper extrema,
matched **source-row ID pairs**, and complete NOT TRUE pairs (including
SQL UNKNOWN). The matched/rejected pairs partition the Cartesian source
bag exactly; omitted witnesses and independently sampled subsets cannot pass.
All five SQL statements are also checked through the canonical AST and
cross-dialect analysis pipeline, rather than assuming parsing equals proof.

The four-/one-/six-row witnesses in the canonical physical planner derive
the complete matched key-group populations for **two distinct physical input
instances**. Two-key `join_population.additional_keys` applies all equality
components to one shared complete key-group assignment, and the full contract
is emitted in canonical JSON with matching schema definitions. Typed physical
source evidence and non-NULL key values are required; a missing or opaque type
is not silently certified.

**Soundness boundary:** the two zero-output cases are **impossible on their
specified complete physical input bags**, not globally Impossible for every
database instance. A SQL query with an additional `ON l.a > 10` clause can
produce positive rows from other data. The frozen v1 classification was
over-broad; v2 makes this scope explicit without changing any mandatory
fixture ID, SQL, input bag or expected output. The protocol preserves
Residual when there is insufficient general source-level proof, rather than
promoting a fixture outcome to an unqualified global law.

## Original child acceptance mapping

| AC | Verdict | Direct verification |
| --- | --- | --- |
| #1 Freeze immutable named cases and all typed SQL/AST/dialect/adapter/output facts | **Pass** | v2 matrix: two fixture IDs, five variants, source/column types, source instances, closed bags, output domains, every exact input/output row, explicit dialect classifications; `task_109_acceptance_freeze` validates presence and coverage-manifest links |
| #2 Exact canonical binary INNER construction and correct proof boundaries | **Pass** | `task_109_composite_join`, `task_109_frozen_oracles::frozen_positive_counts_have_exact_closed_source_witnesses`: shared typed physical-source bags, 1/4/6 counts, composite equality, proof strength, and honest fixture-specific zero versus unsupported Residual |
| #3 Independent executable source and terminal oracles | **Pass** | `task_109_frozen_oracles::every_frozen_variant_executes_complete_physical_and_terminal_bags`, plus dedicated NULL/composite/duplicate DuckDB tests; literal expected row identities and both SQL truth directions |
| #4 Dialects/adapters/conditional dispositions | **Pass** | Five statements by 13 canonical dialects (65 cells), plus `postgres` alias; canonical AST cross-dialect equivalence; direct SQL, compiled dbt and ODCS physical metadata paths; four named and executed conservative conditional cases |
| #5 Final public tests, formatting, strict lint, full tests, docs, no-default, dbt E2E, protocol/schema | **Pass (implementation)** | Nine checked jobs for implementation HEAD linked above; final metadata-head checks must also be green before PR is ready |

## Conditional review

These **nonmandatory** boundaries have individually named, test-enforced
typed Residual results and concrete missing-evidence explanations in the
freeze under owner TASK-109:

- `J109-COMPUTED-ON`: no attested physical arithmetic-key inverse.
- `J109-NULLSAFE`: SQL NULL-matched partner law not implemented by ordinary `=`.
- `J109-OR`: disjunctive join cardinality requires a separate union/overlap law.
- `J109-STRING`: missing physical string comparison/collation equivalence.

They are accepted as fail-closed **for this child scope**, not as claims of
native vendor/session behavior. Non-AND, NULL-safe, non-equality and
vendor-specific join families remain with TASK-113/114 and global
TASK-88/91 review. TASK-110/115/116 retain chained topology, cross-terminal
composition and full TASK-71 parent release verification.

## Standards/spec review

No production dialect whitelist or consumer SQL reparsing was added. The
schema/constructor explicitly admits at most one additional key and only
the finite binary INNER shape. All public integration tests exercise the
analysis/planner or canonical JSON boundary. The unknown-as-exact,
key-filter, ambiguous lineage and cross-join-composition risks are guarded
by conservative residuals rather than speculative assignments.
`JoinPopulation` adds a public optional wire property and a public Rust
variant field; it is a versioned **breaking** protocol/library API change
intended for the planned v3 release and must be classified accordingly.

**Handoff:** TASK-109 is ready to merge after final CI. TASK-110 remains
blocked until this PR is actually merged into `main`, even if its task
file says Done on this PR branch. TASK-71 remains In Progress.
