# TASK-177 acceptance audit, bounded evidence and release blockers

**Task:** [TASK-177](../.backlog/tasks/task-177%20-%20freeze-task-66-feature-by-13-dialect-parser-coverage.md), TASK-88 child 1/4.
**Implementation:** [PR #196](https://github.com/phdah/sql-semantic-protocol/pull/196).
**Disposition:** **In Progress**. This is not independent acceptance signoff.

## Independent evidence boundaries

The versioned [TASK-177 acceptance freeze](task-177-acceptance-freeze.json) retains
all **251** original TASK-66 named variants and **13** exposed dialect names,
giving **3,263** distinct cells. Baseline fixture attribution remains
**156 fixture-tested, 105 representative-only, 3,002 unverified**; it cannot
be silently promoted to semantic proof.

The [SQL sample registry](task-177-parser-samples.json) binds one explicit
SQL example to each original variant and its original TASK-66 semantic owner.
The [parser/analyzer snapshot](task-177-parser-snapshot.json), exercised by
[`task_177_parser_matrix.rs`](../tests/task_177_parser_matrix.rs), records:

| Result of 3,263 examples | Cells |
| --- | ---: |
| sqlparser accepts input | 3,208 |
| sqlparser rejects input | 55 |
| Public analyzer returns a protocol document | 3,092 |
| Public analyzer returns an explicit Unsupported statement | 116 |
| Public analyzer errors after parsing | 0 |

The 55 parser-rejected sample/dialect pairs have named, unapproved
[syntax boundaries](task-177-parser-boundaries.json). The 116 analyzer
Unsupported pairs have named semantic owners and blocking
[analysis boundaries](task-177-analysis-boundaries.json).
A parser rejection is a limitation of **that exact sample**, not evidence
that the dialect lacks every alternative feature syntax.

The separate [full canonical protocol comparison](task-177-equivalence-snapshot.json),
exercised by
[`task_177_canonical_equivalence.rs`](../tests/task_177_canonical_equivalence.rs),
compares the emitted public protocol for the **same SQL sample** across all
13 dialect parsers, ignoring only the explicit input-dialect metadata.
Its exhaustive cell classification is:

| Complete public protocol comparison | Cells |
| --- | ---: |
| Same serialized protocol as the first analyzed dialect | 3,069 |
| Different serialized protocol | 23 |
| Explicit Unsupported | 116 |
| Parser rejected | 55 |
| Public analysis error | 0 |

The **23** document differences affect **five SQL samples**, including
ROLLUP, CUBE, GROUPING(), correlated LATERAL and STRUCT CAST. Every pair's
exact first differing public JSON path, reference dialect, source SQL,
semantic owner, required review and **unapproved** release disposition are
pinned in the [drift register](task-177-canonical-drift-boundaries.json).
The earliest divergences occur in grouped zero-output physical-source cases,
one LATERAL witness reason and one STRUCT diagnostic. This is field-level
triage evidence, **not** a reviewed explanation that the dialect laws differ.

## Native physical oracles, adapter facts and evidence ledger

The [case-level physical proof ledger](task-177-physical-proof-ledger.json)
independently reconciles the 251 original TASK-66 sample identities against
their **actual first-party native and typed-oracle owners**, without promoting
missing proofs. It records **227 distinct original SQL examples** with original-SQL native DuckDB source and terminal outputs or poststate oracles, and **24** that still lack one.
Two separate TASK-177 positive/Impossible goldens are not among the 251 sample
IDs. The different native suites overlap on three ROLLUP/CUBE/GROUPING cases;
overlapping executions must not inflate unique proof counts.

- `88-C177-P` and `88-C177-N` remain the two separate positive and
  Impossible goldens with full physical input/terminal/NOT TRUE/UNKNOWN bags.
- The [25 original predicate cases](task-177-predicate-oracles.json)
  retain complete NULL/duplicate source, truth partitions and terminal bags;
  ten of them have strongest typed interval/set and direct SQL/dbt/ODCS parity
  checks in [the adapter test](../tests/task_177_predicate_adapter_parity.rs).
  dbt view execution remains `Unattested`.
- The [five drift native cases](../tests/task_177_cross_dialect_native_oracles.rs)
  check actual grouped, LATERAL and STRUCT output shapes and occurrence bags.
  This **does not** resolve the 23 dialect canonical drifts.
- The [98 additional native exact-SQL fixture cases](task-177-relational-bag-oracles.json)
  execute original SQL in DuckDB against literal, complete source bags with
  nullable identifiers and duplicate occurrences. The
  [executable tests](../tests/task_177_relational_bag_oracles.rs) cover
  joins, UNION/INTERSECT/EXCEPT, grouping sets, windows/QUALIFY,
  subqueries, CTE/derived scopes, pagination and table functions.
  Three grouping cases overlap the five drift oracles.
- Five additional native original-SQL oracle suites, with separate versioned
  source/terminal evidence fixtures, add 89 unique cases:
  [query, ordering, PIVOT and multi-terminal oracles](task-177-native-expansion.json),
  [DML before/after and RETURNING bags](task-177-write-oracles.json),
  [string/temporal/signed-zero cases](task-177-predicate-environment-oracles.json),
  [DDL/constraint/transaction state](task-177-schema-lifecycle-oracles.json)
  and [window ranking/frame results](task-177-window-oracles.json).
  They execute the original frozen SQL and use independent literal physical
  inputs and expected complete outputs. A valid native fixture does not itself
  certify strongest value domains or cross-adapter equivalence.
- Three additional native negative cases (overflow, incompatible UNION arity,
  multi-valued scalar subquery) are kept separate from the 227 completed
  original SQL terminal/state oracles.
- Three exact SQL samples expose **DuckDB native syntax boundaries**:
  `88-C177-S054` (LEFT SEMI), `88-C177-S055` (LEFT ANTI), and
  `88-C177-S160` (FETCH FIRST WITH TIES). Native alternatives have
  independently pinned outputs for their fixtures, but those alternatives
  do **not** certify execution of the original syntax or vendor portability.
  `88-C177-S083` (UNION BY NAME) executes natively but its canonical
  name-based column alignment remains explicitly unsupported and diagnostic,
  not an empty two-column semantic proof.
- [The fail-closed ledger test](../tests/task_177_physical_proof_ledger.rs)
  ensures 251 unique rows, 227 native original examples, 24 missing
  examples, ten narrower typed adapter cases and **zero** independent signoffs.

Four additional independently specified typed outcomes (NOT EQUAL with NULL excluded, NOT BETWEEN as two open intervals, AND and parenthesized OR/AND with positive ID bounds) now have direct SQL, dbt and ODCS parity assertions. They expand typed parity from six to ten without treating other output domains as exact.

These are concrete incremental oracles, **not** 251 fully certified canonical
typed-domain, rejected-direction, SQL/dbt/ODCS and vendor/session cases.

## Latest native deterministic and conditional evidence

The exact original SQL deterministic native oracle suite adds **13** distinct cases (pattern matching, collation, NaN, scalar aggregates, typed scalar results, enum catalog state and UTC session state). Its source and terminal/poststate evidence is guarded in the case-level ledger; fixed native rejection and stochastic-control probes do not inflate completed counts. The full typed-domain, adapter, vendor/session and independent signoff obligations remain blocked.

The new [bounded native probes](../tests/task_177_native_boundary_probes.rs) execute the original rejected syntax with complete source relations and classify stochastic sampling, recursion and RANDOM without inventing fixed outputs. Three original samples (S176, S181, S242) remain conditional native candidates. S181's geometry cast executes in the pinned DuckDB session, contradicting the earlier rejection assumption; its exact typed terminal proof remains missing. **Neither test passing nor an explicit rejection approves release Residual or Impossible status.**

## Original child acceptance criteria

| AC | Executable evidence | Remaining blocker |
| --- | --- | --- |
| #1 Complete finite freeze | 251 stable sample IDs, 3,263 parser and analyzer cells, full canonical comparison, explicit source tasks and boundaries | Complete independently reviewed typed-domain, full physical source/terminal and P/N/Impossible/Residual evidence is still absent for most mandatory variants, including combined synonyms and alternative syntax forms |
| #2 Sound outcome semantics | 25 exact NULL/duplicate predicate membership oracles, ten typed range/set claims, two original P/N goldens, five native grouping/LATERAL/STRUCT oracles | No independent strongest-domain and cross-source correlation certification for every remaining family or every parsed dialect; matching protocol documents can still contain Unknown or incomplete outcomes |
| #3 Native differential proof | 227 distinct original SQL sample source/terminal or state oracles plus two separate positive/Impossible goldens; three-valued rejection, duplicate amplification, group subtotals, ranking and subquery identities | 24 original SQL samples lack independent native complete-bag or state oracle; many executed cases also lack original rejected-direction and strongest canonical typed-domain proofs |
| #4 Dialects and adapter parity | Full 3,263-cell structural comparison, named 23-drifts/55-syntax/116-Unsupported boundaries, ten SQL/dbt/ODCS typed-domain fact equivalences | Cross-dialect semantic divergences need reviewed resolution or explicit approved conditional laws; three new DuckDB exact-syntax rejections are recorded. Remaining representable adapter facts need parity tests and other native vendor/session execution remains unattested |
| #5 Full verification, pinned consumer and independent signoff | GitHub Actions runs Rust default/no-default tests, fmt, strict Clippy, docs, dependencies and dbt Core E2E on each PR head | Draft consumer PR #47 records candidate-pinned sql-tdg compatibility checks. After every protocol head change, re-pin and rerun all five consumer checks against the exact final SHA; an earlier green run does not verify a newer candidate. Independent per-cell acceptance/release review is still missing |

## Rules for releasing or signing off

1. **Parsing is not proof.** `C` means only matching full public JSON
   documents for one concrete SQL example. It does not prove exact semantics
   or native vendor execution.
2. **Unknown is not Exact.** Preserve conservative value domains,
   full-source multiplicities and cross-source correlations. Every native
   proof needs independently expected output, positive and rejected
   membership and the same complete physical sources.
3. **Conditional means reviewed.** A Residual/Impossible/vendor/session
   boundary is not an approved release deferral until a reviewer checks
   its evidence, owner, assumptions and consequence.
4. **Source-independent canonical semantics.** SQL, dbt and ODCS must
   agree for every equally representable fact. An unattested adapter
   execution cannot be promoted to a completed materialization.
5. **Parent and consumer remain separate.** TASK-178 is blocked until
   this child meets its own AC and is merged. TASK-88 and TASK-91 are
   independent parent/release gates. The protocol repository owns the
   canonical suite; the sql-tdg **consumer pins this protocol candidate**,
   never the other way around.

## Author-side standards/spec assessment

**Standards:** Only tests, fixtures, documentation and bounded CI
reliability logic change. Public protocol, crates, emitted schema and
production domain semantics remain unchanged. Exact first-difference
paths and source-backed expected rows guard against treating parser
coverage as native semantics. Native engine tests use public library
entry points and explicit physical inputs.

**Spec:** The finite parser and canonical comparison freeze is complete
as an observation, but independently certified semantic/physical and
adapter equivalence for all mandatory families is **not**. The remaining
AC require additional first-party oracle implementations and external
review. This author-side assessment cannot replace an independent reviewer.
