---
id: TASK-91
title: Block 3.0.0 release until downstream generator acceptance
status: To Do
assignee: []
created_date: '2026-10-09'
updated_date: '2026-10-09'
labels: []
milestone: m-3
dependencies: 
  - TASK-66
  - TASK-67
  - TASK-68
  - TASK-69
  - TASK-70
  - TASK-71
  - TASK-72
  - TASK-73
  - TASK-74
  - TASK-75
  - TASK-76
  - TASK-77
  - TASK-78
  - TASK-79
  - TASK-80
  - TASK-81
  - TASK-82
  - TASK-83
  - TASK-84
  - TASK-85
  - TASK-86
  - TASK-87
  - TASK-88
  - TASK-89
  - TASK-90
references: 
  - 'sql-tdg TASK-24..36'
  - 'PR #79'
priority: high
type: task
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
One consolidated 3.0.0 release is requested. The existing Release Please PR #79 must remain unmerged until the generator-relevant protocol contract is complete and verified.

**Release contract:** This task is a blocking prerequisite for the single protocol 3.0.0 release and sql-tdg milestone m-3. Implement canonical, source-independent, typed obligations; do not reparse SQL in the consumer. Preserve strongest safe value domains through composition, and distinguish exact, impossible and residual for positive and negative cases. Arbitrary unsupported behavior must fail closed and appear in the audited capability matrix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 All protocol TASK-66..90, TASK-68 children TASK-93..100, and release-blocking coverage-manifest entries are Done; no unchecked acceptance criteria or undocumented exclusions remain.
- [ ] #2 Candidate Rust fmt/clippy/test/doc, dbt Core E2E, no-default-features, schema snapshots, dialect and cross-feature differential tests are green.
- [ ] #3 sql-tdg integration branch consumes pinned *protocol release-candidate Git SHA*, with no dependency on an already published 3.0.0 crate, and passes generator TASK-24..31, TASK-35 and TASK-36 acceptance where upstream-dependent.
- [ ] #4 Decide and document final scope for arbitrary UDFs, stochastic operators, nonterminating recursion and unavailable vendor engines with explicit fail-closed behavior. No universal-support claims.
- [ ] #5 Maintainer signs off complete feature/dialect matrix and final dbt Makefile workflow before PR #79 is made ready/merged. Only then publish v3.0.0 once and switch sql-tdg to the published crate.
- [ ] #6 Add unit, cross-dialect and differential tests proportional to the feature, including feasible/impossible/NULL/duplicate/residual cases, and update API, protocol JSON schema, docs and relevant adapter paths.
- [ ] #7 Verify CI for each supported dialect's shared and dialect-specific SQL equivalence to the **same canonical semantic outcomes** where meanings are equivalent, including safe conditional session laws, while DuckDB E2E executes the complete supported transformation and DML/DDL sequence.
- [ ] #8 Verify seeded randomized rejected predicate/column alternatives, multi-seed coverage of every feasible alternative, and exact per-terminal presence/absence against complete generated DuckDB output snapshots.
<!-- AC:END -->


## Reviewed finite acceptance contract (2026-10-16)

The existing **AC #1..#8 remain required**. Interpret and verify them through the frozen TASK-91 row of [the backlog acceptance contracts](../../docs/backlog-acceptance-contracts.md), additive to the approved TASK-66 feature/dialect inventory. This specifies a finite **minimum exactness floor**, not permission to omit named feature families or mark unimplemented mandatory cases `Residual`.

1. **Mandatory exact core:** Hard gate: every required TASK-66..90 and TASK-93..108 sign-off has its approved matrix; no unchecked blockers or unowned coverage cells; TASK-92 remains post-v3.
2. **Mandatory cross-operator / adapter evidence:** Hard gate: 13-dialect parser/canonical CI, schema snapshots, full DuckDB/dbt scripted oracles, exact/impossible/residual outputs and all required Rust checks on one pinned release-candidate SHA.
3. **Conditional and non-exact boundary:** Hard gate: sql-tdg TASK-24..31/35/36 complete with multi-seed negative-alternative coverage; maintainers approve deferrals and engine evidence before Release Please PR #79.

**Acceptance sign-off:** Before implementation, register each original named AC variant as a versioned feature/dialect cell and freeze fixture IDs in `docs/coverage-manifest.json`. Every mandatory core cell needs typed output domains (including NULL and interval inclusivity), positive/rejected membership and `Exact`/`Impossible` physical-source proof as applicable, complete DuckDB source/terminal oracles where executable, and representable SQL/dbt/ODCS parity plus 13-dialect parser equivalence evidence. Conditional cells need a documented parser/runtime boundary, typed `Residual` or justified `Impossible`, named owner, and explicit release disposition, **not a shortcut for missing implementation**. Sign off AC #1..#8 only after required cells, downstream sql-tdg integration, maintainer release approval and final CI are verified. TASK-91 itself is the independent release gate; documentation of a conditional residual alone cannot authorize protocol 3.0.0.

## Delivery guidance

Implement in the protocol repository before releasing 3.0.0. Do not solve missing protocol facts through sql-tdg heuristics. Update the machine-readable coverage manifest and cross-repo dependency map in TASK-66/91. Independent implementation PRs may land on main while 3.0.0 remains held; no intermediate releases are required.

## TASK-70 parent decomposition and protocol release gate (2026-10-13)

PR #108 is a mergeable **bounded foundation** only. TASK-70 remains In Progress with its six original AC unchecked. Its dependency-ordered implementation children **TASK-101 → TASK-102 → TASK-103 → TASK-104 → TASK-105 → TASK-106 → TASK-107 → TASK-108** own arithmetic/conversions, computed branches/aliases, correlated Boolean proofs, dialect-sensitive text, temporal/JSON/vendor residual laws, relational/physical composition, expression-specific oracles, and final parent signoff. See the queue in [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md). The existing TASK-91 dependency on **TASK-70** indirectly gates all eight children. Do not add child prerequisites on TASK-70 (cycle), merge Release Please PR #79, or approve protocol 3.0.0 until TASK-108 verifies the original six parent criteria **and** all independent TASK-88/89/90, sql-tdg and release AC are satisfied.

## TASK-68 parent decomposition (2026-10-10)

TASK-68 remains In Progress after foundational PR #92 is merged. Its child
execution queue is TASK-93 → TASK-94 → TASK-95 → TASK-96 → TASK-97 → TASK-98
→ TASK-99 → TASK-100, with sequential dependencies and all eight included
in parent closure requirements. TASK-100 verifies the original six TASK-68
criteria, rather than treating the narrower PR #92 as task completion. This
queue does not waive existing TASK-70..90 local-operator, dialect or adapter
work, nor sql-tdg TASK-24..36 or this TASK-91 release gate.

## Coverage manifest and cross-repo gate (2026-10-09)

The executable coverage inventory lives in [docs/coverage-manifest.json](../../docs/coverage-manifest.json), with its readable [dialect matrix](../../docs/coverage.md) and [Rust fixtures](../../tests/coverage_manifest.rs). TASK-66 is Done as the reviewed inventory and routed gap list, **not as proof of all variants**; TASK-88 owns exhaustive parser/canonical dialect laws; TASK-89 owns cross-feature oracle and dbt fixture certification. Each sql-tdg TASK-24..31/35/36 maps to upstream TASK-67..90 in the matrix. sql-tdg TASK-43 must pin a protocol **Git commit SHA** before protocol 3.0.0 is published.

**Gate remains closed** for all cells marked unverified, operator-local, residual, or conditionally deferred behavior without a documented fail-closed proof. The maintainer has **approved** sql-tdg TASK-35 per-terminal rejection with seeded randomized rejecting alternatives, TASK-36's unified dbt `make all` plus required scripted DML/DDL gate, and conditional future deferral for unproved opaque behavior. Implementation, canonical equivalence across all supported dialect/variants, and full executable release evidence remain pending.

**Maintainer scope choices:** [docs/coverage-signoff.md](../../docs/coverage-signoff.md). They cover sql-tdg TASK-35's negative row meaning, TASK-36's DML test location, future-extensible opaque/unbounded deferrals and their fail-closed tests, and canonical versus native-engine certification. A scope approval is **not** final release approval.

## Approved decisions versus release approval (2026-10-09)

[The four maintainer scope decisions](../../docs/coverage-signoff.md) are approved, **not** final release approval. Protocol TASK-66..90, all variant/dialect equivalence cases, the seeded multi-terminal negative contract, physical-source constructive proofs, and sql-tdg TASK-31/35/36 whole-project dbt `make all` plus scripted DuckDB DML/DDL must finish before final TASK-91 sign-off or Release Please PR #79 merge. Safe future UDF/recursion/stochastic extensions tracked in TASK-92 are not permanently prohibited and are not v3 blockers unless explicitly advertised as supported.

## Inventory completion does not imply release acceptance

TASK-66 was closed as the 53-family, 251-variant, 13-dialect **inventory and ownership handoff** in protocol PR #88. Its status must not be used to infer that an unverified manifest cell has passed. TASK-67..90, sql-tdg TASK-24..31/35/36/43/44 and the executable final TASK-91 release gate are still required; do not merge Release Please PR #79 based on TASK-66 alone.

## Protocol-owned conformance corpus and consumer gate (2026-10-12)

The protocol repository is authoritative for `tests/fixtures/conformance/v1/`
and the portable `suite.json` inventory and expected outcomes. Its
physical SQL tests are `tests/protocol_conformance_physical.rs` and
`tests/protocol_conformance_dialect_laws.rs`; the eleven-model dbt DuckDB
gate is `tests/protocol_conformance_dbt_e2e.rs` under `make dbt-e2e`.
See `docs/conformance-suite.md` and
`docs/coverage-manifest.json.task_99_protocol_conformance_contract`.
The original sql-tdg fixture SHA is historical provenance, **not** a
protocol dependency.

sql-tdg TASK-43 must pin the **protocol** Git SHA (never the reverse),
consume the canonical fixture suite and validate its own generated output
without consumer-side SQL reparsing. TASK-31/35/36 DML/DDL generation and
randomized negative output oracles remain pending. This suite does **not**
unblock protocol 3.0.0.

## TASK-68 bounded composition sign-off (2026-10-12)

TASK-93..100 close the bounded protocol-side TASK-68 proof queue in
PR #106. Its formerly residual positive filtered JOIN and singleton
GROUP/HAVING terminal goals now have exact complete physical-source
assignments, verified jointly with DuckDB, adapter and cross-dialect
evidence in [docs/task-68-signoff.md](../../docs/task-68-signoff.md).
This **does not** certify arbitrary feature/operator combinations or
downstream generated source rows. The independent TASK-70..90
dialect/feature matrix, consumer TASK-31/35/36/43 and TASK-91 release
acceptance remain open. Release Please PR #79 must not be merged.

## TASK-79 Boolean CHECK and typed INSERT progress (2026-10-13)

PR #107 now models canonical SQL three-valued Boolean CHECK trees, shared by SQL DDL and dbt, and proves bounded nested typed `INSERT ... SELECT` using full physical source/target snapshots. These are not general function, coercion, transaction or arbitrary nested DML proofs. All remaining unverified dialect/engine/consumer cells and TASK-79 AC stay release-blocking until independently certified.

## TASK-79 physical-schema proof progress (2026-10-13)

PR #107 additionally proves complete composite comparisons despite unknown earlier components, supports a bounded portable generated function/column dependency DAG, and exports exact typed physical populations as independently validated sidecars. The sidecar is not evidence that the SQL statements needed to produce those Arrow values execute under every target engine. TASK-79's remaining unproved typed SQL DML/operator laws, adapter enforcement laws and dialect-oracle cells stay release-blocking, without consumer-side reconstruction or waiver.

## TASK-79 schema proof gate (2026-10-13)

TASK-79 implementation is under review in protocol PR #107, and remains a mandatory dependency for this release gate. Its canonical/schema facts, typed nested populations, generated/default subset, dbt and SQL CHECK parity, scalar equality and differential fixtures do **not** constitute final AC signoff. Identity-sequence allocation, deferred FK/transaction enforcement, fully proven coercion/collation/timezone/default adapters, nested Arrow ordered DML/serialization and the complete cross-engine/dialect matrix remain release blockers or require independently reviewed fail-closed certification. The manifest's `task_79_schema_evidence` scope is authoritative; sql-tdg may consume certified portable protocol facts but must not fill unknown protocol semantics with generator heuristics. Keep Release Please PR #79 and this TASK-91 gate closed.

## TASK-70 bounded expression evidence and remaining gates (2026-10-13)

PR #108 contributes typed local source-row truth for NULL membership, exact
signed integer predicates, certified total CAST/TRY_CAST/SAFE_CAST, selected
COALESCE/NULLIF/CASE NULL tests, and binary-attested safe LIKE prefixes.
The matrix under
`docs/coverage-manifest.json.task_70_predicate_evidence.family_evidence`
contains the exact proof preconditions and explicitly release-blocking residual
families. It does **not** certify general arithmetic, dialect-sensitive
coercions/collations/regex, date/time/JSON/vendor semantics, arbitrary
computed joins/groupings/writes, or all physical positive/rejecting populations.
TASK-88/TASK-89, sql-tdg TASK-24..36/43/44 and this TASK-91 gate remain open,
and PR #79 must not be merged solely from this progress.

## TASK-108 completed handoff (2026-10-17)

TASK-70 and TASK-108 completed their expression-specific acceptance on protocol PR #116. TASK-91 remains publication-blocking until TASK-88/89/90 and sql-tdg end-to-end evidence are separately signed off. No universal/vendor runtime equivalence or 3.0.0 release is approved.


## Dependency-ordered implementation gates (2026-10-17)

Original TASK-91 release AC are **unchanged**. The 19 still-open
implementation parents TASK-71..78 and TASK-80..90 now depend for **closure**
on their 80 bounded child tasks TASK-109..188. See
[`docs/backlog-child-queue-plan.json`](../../docs/backlog-child-queue-plan.json)
and [the queue audit](../../docs/backlog-acceptance-contracts.md).
Do not add every child as a direct TASK-91 dependency: dependency closure via
each parent is authoritative and avoids duplicate synchronization.

Before releasing 3.0.0, verify all child tasks Done/merged and all parent
original AC independently signed off, per-child exact and conditional coverage
fully evidenced, TASK-88/89/90 complete, downstream sql-tdg pinned acceptance
green, and Release Please PR #79 held until the existing TASK-91 gates pass.
TASK-79 is already completed, while TASK-92 is explicitly post-v3.
