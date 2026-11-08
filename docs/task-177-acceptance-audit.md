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

## Native physical oracles and adapter facts

- `88-C177-P` and `88-C177-N` remain the original positive and
  Impossible DuckDB goldens with complete physical source, output, NOT TRUE
  and UNKNOWN bags and two portable bounded-domain assertions.
- The [25 additional predicate cases](task-177-predicate-oracles.json),
  exercised by
  [`task_177_predicate_oracles.rs`](../tests/task_177_predicate_oracles.rs),
  pin one complete NULL/duplicate source, seven concrete customer rows,
  all full original SQL terminal bags and TRUE/FALSE/UNKNOWN/NOT TRUE
  partitions. Six specified portable samples assert strongest typed
  singleton or inclusive/exclusive interval domains in the public
  protocol and across all 13 parser dialects.
- The [SQL/dbt/ODCS parity test](../tests/task_177_predicate_adapter_parity.rs)
  verifies those six exact typed metadata facts where the adapters can
  provide them. dbt view materialization execution remains `Unattested`.
  Previously reviewed TASK-176 and TASK-173 adapters remain valid only
  for their own explicitly certified source shapes.
- The [five canonical drift native oracles](../tests/task_177_cross_dialect_native_oracles.rs)
  execute the *original* grouping, LATERAL and STRUCT sample SQL against
  independent, complete DuckDB physical inputs, including nullable group
  identifiers, duplicate keys, subtotal counts and structural field values.
  Their **DuckDB** execution does not certify 13 vendor runtimes or the
  correctness of the other dialect parser branches.

This amounts to **32** task-owned original-or-explicit P/N native DuckDB
sample cases, not 251 complete typed semantic certifications.

## Original child acceptance criteria

| AC | Executable evidence | Remaining blocker |
| --- | --- | --- |
| #1 Complete finite freeze | 251 stable sample IDs, 3,263 parser and analyzer cells, full canonical comparison, explicit source tasks and boundaries | Complete independently reviewed typed-domain, full physical source/terminal and P/N/Impossible/Residual evidence is still absent for most mandatory variants, including combined synonyms and alternative syntax forms |
| #2 Sound outcome semantics | 25 exact NULL/duplicate predicate membership oracles, six typed range/set claims, two original P/N goldens, five native grouping/LATERAL/STRUCT oracles | No independent strongest-domain and cross-source correlation certification for every remaining family or every parsed dialect; matching protocol documents can still contain Unknown or incomplete outcomes |
| #3 Native differential proof | Complete DuckDB source and terminal bag comparisons for the 32 named cases; three-valued rejection, duplicate amplification, group subtotals and LATERAL identities | Need original complete physical/terminal and output-domain proofs for the other mandatory SQL variants, not parser acceptance, sample count or a cached internal recomputation |
| #4 Dialects and adapter parity | Full 3,263-cell structural comparison, named 23-drifts/55-syntax/116-Unsupported boundaries, six SQL/dbt/ODCS typed-domain fact equivalences | Cross-dialect semantic divergences need reviewed resolution or explicit approved conditional laws, and representable adapter facts require the remaining family-level parity tests; native vendor/session execution remains unattested |
| #5 Full verification, pinned consumer and independent signoff | GitHub Actions runs Rust default/no-default tests, fmt, strict Clippy, docs, dependencies and dbt Core E2E on each PR head | A candidate-SHA pinned sql-tdg conformance run and independent per-cell acceptance/release review have not been supplied; do not self-certify completion based on CI alone |

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
