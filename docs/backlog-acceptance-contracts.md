# Bounded backlog acceptance contracts (TASK-70, 71..92, 107..108)

This review freezes a **finite verification floor**, not a ceiling on protocol features. It complements the existing acceptance criteria and the TASK-66 feature/dialect inventory. Each task's existing AC remains mandatory; none is waived by this review.

**Snapshot boundary:** Feature-family membership is fixed by this review and the approved `docs/coverage-manifest.json` inventory at baseline commit `b75b16a29234fc207af0014bf662d39223b2502f`. The eventual implementation task registers specific fixture IDs before changing semantics; adding newly discovered SQL/operator variants later does not silently expand that task. Mandatory core families listed here cannot be reclassified as residual solely because implementation is difficult. If the inventory genuinely needs changing, revise the task contract explicitly in a separate reviewed change and retain the required release-blocking richness.

## Applying bounded contracts to each backlog task

The per-task table below is a **minimum scope floor**, not a completed
fixture-by-fixture acceptance matrix. Updating TASK-107 does not implicitly
freeze TASK-108, TASK-71..90, TASK-91 or any other unfinished task. Before
each task is implemented, freeze its *own* immutable inventory of expected
SQL/dialect/adapter cases and executable signoff assertions, linked from that
task and the canonical coverage manifest. The repository's `AGENTS.md` now
makes this a default prerequisite for all future implementation work. Completed
tasks do not need retroactive ticket rewrites solely for this policy.

**TASK-107 reference:** `docs/task-107-acceptance-freeze.json` fixes 42
TASK-66 predicate variants, 13 dialects (546 individually identified
parser/canonical classification cells), all 14 TASK-70 child-family obligations,
9 concrete source/output golden cases, 9 composition pairings, four random
seeds, and five explicit AC gates. Unverified dialects/cases are blockers, **not**
test passes. All original TASK-107 acceptance criteria remain mandatory; the
matrix pins their finite interpretation without accepting unimplemented exact
features. Its `tests/task_107_acceptance_freeze.rs` contract checks
intentionally validate scope rather than pretend to certify semantics.

## Release-quality completion rules

1. **Inventory closure:** Before implementing a task, map every explicitly named operator, syntax variant, proof and adapter in its original AC to a versioned matrix cell: `exact_required`, `conditional` (with evidence prerequisites), or `approved_deferred` (explicit owner, reason, follow-up and release approval). No implicit `where supported` exclusions. Freeze the matrix and review any scope changes.
2. **Feature-richness floor:** All examples in the task's `Exact core` and `Exact composition` below are mandatory constructive outcomes with typed domains, positive AND NOT TRUE/rejected directions where semantically applicable, complete source populations and independent DuckDB execution for executable SQL. These cases cannot be closed by a residual. Extend with a deterministic fixture for each originally named major operator family even when not shown in one example.
3. **Correct residuals:** Conditional/vendor/opaque forms can be residual only when necessary assumptions, metadata, semantics or a proof rule genuinely cannot be established. Lack of implementation for an advertised mandatory family remains a **blocking gap**, not an acceptable residual. Necessary contradictions are `Impossible`; sound certified constructions are `Exact`/`Feasible`.
4. **Finite combinations:** Implement the explicit core paths and listed operator pairings, not the Cartesian product of arbitrary SQL ASTs. Cross-feature matrix and larger full-project paths belong to TASK-85/89, not every local child.
5. **Traceability:** Each frozen cell records SQL shape, dialect and parser classification, expected value domain (bounds/inclusivity/NULL), proof strength for TRUE/NOT TRUE, stable column/source identities, fixture IDs, expected DuckDB source/terminal rows and parity adapter facts. The canonical coverage manifest must link cells to tests and owners.
6. **Closure:** Every mandatory cell green, every conditional cell classified and reviewed, zero false exactness, CI green. Defer only with explicit maintainer-approved release disposition and assigned follow-up. Parent tasks and release gates remain separately checked.

## Per-task minimum matrices

Each three-item row specifies the **minimum** required certified path, the bounded interaction, and the reviewed conditional boundary. The original task AC still names all other inventory cells to classify.

| Task | Must prove exact / constructive core | Required composition | Conditional and residual edge inventory |
| --- | --- | --- | --- |
| TASK-70 | Parent sign-off: TASK-101..107 and TASK-108 map the six original AC, with each family in the approved TASK-66 inventory classified. | Finite child matrix: signed arithmetic/casts, CASE/COALESCE/NULLIF, correlated predicates, text patterns, temporal/JSON/vendor boundaries, composed physical graphs, SQL/dbt/13-dialect oracle evidence. | TASK-108 must audit every original AC and unresolved family; no parent completion from bounded TASK-106 alone. |
| TASK-71 | Exact core: two- and three-relation INNER equijoins (including one-to-many duplicate keys); LEFT/RIGHT/FULL unmatched NULL-extension; SEMI/ANTI absence; self-join with two aliases. | Exact composition: two-hop join with upstream WHERE and output count, composite equality keys and explicit NULL/rejection examples, all from one closed physical input bag. | Inventory conditional: CROSS, USING/NATURAL, OR/non-equi/null-safe, ASOF/time/range and LATERAL/APPLY by dialect; evidence-backed exact cell or typed residual with owner and release disposition. |
| TASK-72 | Exact core: two-column UNION, INTERSECT and EXCEPT, each DISTINCT and ALL, with duplicate counts and SQL NULL tuple equality. | Exact composition: three-branch nested tree, positional type/alias alignment and at least one shared-source branch/filter with complete physical row obligations. | Inventory conditional: MINUS, BY NAME/CORRESPONDING, coercion/collation, branch/set ORDER/LIMIT and nontrivial join/group branches; exact only with proven tuple histograms. |
| TASK-73 | Exact core: COUNT(*), COUNT(expr), SUM, MIN, MAX, AVG with typed result domains and NULL/duplicate behavior, both grouped and global empty-input cases. | Exact composition: one filtered join feeding GROUP BY/HAVING with two aggregates, DISTINCT or FILTER, and a verified positive and rejected group. | Inventory conditional: GROUPING SETS, ROLLUP, CUBE, GROUP BY ALL and vendor GROUPING syntax; each has a parser/evaluation classification and representative oracle. |
| TASK-74 | Exact core: ROW_NUMBER, RANK, DENSE_RANK on explicit total or tied ordering; LAG/LEAD, FIRST/LAST_VALUE and a bounded ROWS frame, retaining NULL and peer semantics. | Exact composition: PARTITION BY plus QUALIFY and one upstream filter/join or aggregate on a complete known bag, with both survivor and rejected output. | Inventory conditional: NTILE, RANGE/GROUPS frames, COLLATE and engine-default NULL ordering; unproven ties/order/session assumptions must not claim exact ranks. |
| TASK-75 | Exact core: correlated and uncorrelated EXISTS/NOT EXISTS, IN/NOT IN with NULL UNKNOWN, and scalar subquery zero/one/multiple rows. | Exact composition: correlated one-key and composite-key membership through a join or grouped inner query with explicit outer-row identities and rejected/absence proofs. | Inventory conditional: ANY/SOME/ALL, nested correlation and LATERAL/APPLY parser variants, with multiplicity and engine ambiguity explicitly captured. |
| TASK-76 | Exact core: deterministic ORDER BY plus LIMIT/OFFSET/FETCH/TOP, WITH TIES and DISTINCT/DISTINCT ON where stable ordering is demonstrated. | Exact composition: two nested pagination boundaries (CTE or set branch) with precise count and survivor/rejected outcomes, including empty input. | Inventory conditional: TABLESAMPLE/SAMPLE, engine NULL/tie defaults and QUALIFY placement; stochastic or underordered membership remains residual with typed reasons. |
| TASK-77 | Exact core: VALUES multirow columns, deterministic array UNNEST/EXPLODE with NULL/empty and WITH ORDINALITY when parsed, plus supported SELECT star EXCEPT/REPLACE/RENAME. | Exact composition: one deterministic fanout through join or aggregation with preserved source-instance and result types; PIVOT/UNPIVOT fixed-schema values tested. | Inventory conditional: dynamic JSON tables, opaque generator/table functions, variable pivot schema and vendor-specific expansion, with explicit schema and determinism guards. |
| TASK-78 | Exact core: nested/reused/shadowed ordinary CTEs and derived views with stable physical column identity; materialization distinct from inline scope. | Exact composition: one selectively filtering or computed producer across two consumers, then a downstream join/count or write, proven from original physical inputs. | Inventory conditional: WITH RECURSIVE bounded base/step with termination proof; cycle/unbounded recursion residual; LATERAL, temp objects and multi-statement ordering classified. |
| TASK-80 | Exact core: INSERT VALUES multirow, DEFAULT VALUES, INSERT SELECT, column reorder/omission, defaults and generated columns with exact initial/final bags. | Exact composition: INSERT following CTE with constraints and subsequent SELECT, plus conflict/overwrite/RETURNING on a deterministic portable shape. | Inventory conditional: IGNORE/REPLACE/UPSERT/ON CONFLICT, OVERWRITE scope and vendor conflict resolution; classify nonportable multiple-match/error semantics. |
| TASK-81 | Exact core: UPDATE SET expressions/DEFAULT, DELETE WHERE with TRUE/FALSE/UNKNOWN, preserved untouched rows, target constraints and RETURNING. | Exact composition: one key-unique UPDATE FROM and DELETE USING/CTE with full pre/post snapshots, plus one ordered INSERT/UPDATE/DELETE script. | Inventory conditional: ambiguous multiple-match UPDATE, dialect ORDER/LIMIT, correlated target conditions and engine-specific RETURNING; false exactness prohibited. |
| TASK-82 | Exact core: MERGE matched UPDATE/DELETE, NOT MATCHED INSERT and BY SOURCE handling in ordered first-applicable branches, with full target/source snapshot. | Exact composition: computed assignments and two sequential MERGEs, NULL key and unique vs repeated matches with exact or impossible/residual strength. | Inventory conditional: dialect UPSERT equivalents, ambiguous many-to-one matches and engine-specific branch semantics; exact only with proven match/cardinality constraints. |
| TASK-83 | Exact core: CREATE TABLE/CTAS, CREATE OR REPLACE, VIEW vs materialized VIEW, DROP/TRUNCATE and basic ALTER ADD/DROP/RENAME/TYPE with schema+data snapshot. | Exact composition: CREATE/INSERT/ALTER/SELECT/DROP scripted relation lifecycle, IF EXISTS/NOT EXISTS and lineage to physical sources. | Inventory conditional: CLONE/LIKE, temporary/transient scope and vendor DDL transaction laws; missing runtime/engine semantics stays explicit. |
| TASK-84 | Exact core: deterministic script with INSERT, UPDATE, DELETE, MERGE and table lifecycle, checking complete before/after snapshots at each step. | Exact composition: transaction BEGIN/COMMIT/ROLLBACK and nested SAVEPOINT on a supported DuckDB path, verifying rollback and independent output reads. | Inventory conditional: engine isolation, implicit DDL commits, deferred FK checks and concurrent transactions; no fabricated cross-engine transaction equivalence. |
| TASK-85 | Exact core: one shared physical source satisfying two terminal goals (rows, groups, duplicates/NULL distributions), including an incompatible goal with Impossible, and an unprovable goal with Residual. | Exact composition: bounded join -> filter -> GROUP/HAVING with output frequency and column domains; another joined/grouped or window/set terminal shares the same closed source bag. | Inventory conditional: full tuple histograms, multi-column correlations, LIMIT/QUALIFY/window/set interaction and higher DAG fanout; each enumerated goal needs source/terminal oracle and explicit gap. |
| TASK-86 | Exact core: each source row has per-terminal positive, FALSE, UNKNOWN or absence membership with stable lineage; A OR B rejection requires both arms fail. | Exact composition: two simultaneous terminal filters over shared inputs with at least two distinct feasible rejection columns, seeded deterministic reachability and complete DuckDB terminal snapshots. | Inventory conditional: grouped/window/set rejected alternatives and counterfactuals; record all provably constructive alternatives for frozen fixture predicates, not arbitrary formulas. |
| TASK-87 | Exact core: authoritative dbt manifest compiled SQL/dependencies/constraints and catalog datatypes, direct SQL DDL and ODCS schema mapped to one canonical fact contract. | Exact composition: parity for one table model, one view/ephemeral model and one incremental/write scenario with source identity and typed domain checks. | Inventory conditional: unavailable warehouse introspection, schema-qualified ambiguous names, unknown test enforcement and FK-only metadata, with explicit diagnostics. |
| TASK-88 | Complete inventory: all 13 project dialects crossed with the approved TASK-66 feature/variant IDs and parser statuses; no implicit skipping of parse-capable variants. | Exact equivalence: for each approved shared semantic class assert equal canonical domains, lineage, membership, cardinality and write effects, with minimal AST-specific SQL fixtures. | Conditional boundaries: vendor-dependent NULL order, collation, floating/timezone, coercion, DML match/transaction rules get version/session assumptions; native engine claims only when executed. |
| TASK-89 | Exact core: committed m-3 dbt + scripted DuckDB DML/DDL project with source-row snapshots and positive/negative outputs in every terminal, independently produced by protocol plus pinned sql-tdg. | Exact composition: joins -> groups/HAVING -> window/QUALIFY -> set path and a separate correlated subquery/write path; seeded small-instance and randomized differential checks. | Release evidence: every reviewed TASK-66 mandatory feature cell has an executed fixture or owner-approved conditional deferral; persist SQL, seed, inputs, protocol, outputs and counterexample reasons. |
| TASK-90 | Exact core: public Rust/JSON/CLI contract inventory, stable typed identities/proof strength, deterministic serializer, full schema validation and golden v2 -> v3 changes. | Exact composition: pinned sql-tdg pre-release consumer validates canonical source/witness/write examples against final v3 candidate. | Extensibility: unknown tags and future typed UDF/operation metadata have defined version gates and fail-closed behavior without premature v3 claims. |
| TASK-91 | Hard gate: every required TASK-66..90 and TASK-93..108 sign-off has its approved matrix; no unchecked blockers or unowned coverage cells; TASK-92 remains post-v3. | Hard gate: 13-dialect parser/canonical CI, schema snapshots, full DuckDB/dbt scripted oracles, exact/impossible/residual outputs and all required Rust checks on one pinned release-candidate SHA. | Hard gate: sql-tdg TASK-24..31/35/36 complete with multi-seed negative-alternative coverage; maintainers approve deferrals and engine evidence before Release Please PR #79. |
| TASK-92 | Post-v3 prototype: versioned typed deterministic function signature with inputs, output domain, NULL, error, side-effects, and source cardinality obligations. | Trust evidence: structured user declaration, dbt or catalog/ODCS metadata adapters each show attested vs absent/untrusted evidence with provenance and exact/residual contrast. | Conditional research: bounded recursion, seeded stochastic models and session-sensitive coercion/time/collation classified, not universally exact; prove an extension-safe migration path. |
| TASK-107 | Coverage: each exact expression family delivered by TASK-101..106 mapped to direct SQL, compiled dbt manifest+catalog and typed expected positive/NOT TRUE/domain behavior. | Dialect: all 13 exposed names covered by parser/equivalence classifications, with dialect assumptions for meaning-changing AST; DuckDB seeded boundary/NULL/duplicate/overflow and residual oracles. | Adapters: ODCS/schema fact parity where equivalent metadata is available; no missing expected family, silent residual, unowned deferral or non-reproducible failing seed. |
| TASK-108 | Independent audit: TASK-101..107 Done/merged, original TASK-70 six criteria each mapped to tests/modules/manifest rows and all promised families accounted for. | Evidence: representative exact, impossible and residual expression/physical outcome under SQL/dbt/ODCS, all 13 parser paths and reproducible DuckDB negative/domain oracles. | Gate: full CI and schema snapshots; TASK-70 can close only after all original AC proved; TASK-88/89/91 independent release blockers remain open until their own checks. |

## Explicit ownership

**Parent TASK-70:** Retains its six original AC, may close only via independent TASK-108 after all TASK-101..107 evidence and approved inventory dispositions. TASK-106's bounded matrix does not itself close TASK-70.
**Release TASK-91:** No downgrade of TASK-66 mandatory feature/dialect inventory for convenience. All release blockers, actual producer/terminal oracles, and downstream sql-tdg integration must pass. TASK-92 is explicitly post-v3.
**Completed tasks:** TASK-66..69, TASK-79, TASK-93..106 (as applicable) remain unchanged. This is a forward-looking scope/verification review, not retroactive redefinition of accepted proofs.
**Future extension:** Generalized arbitrary correlated DAGs, nonlinear symbolic inverses, opaque UDFs and unbounded recursion must be tracked as separately bounded tasks or approved conditional cells. A residual is never proof of supported semantics.


## Pre-release child queue audit (2026-10-17, PR #117)

The independently reviewed backlog planning inventory is
[`docs/backlog-child-queue-plan.json`](backlog-child-queue-plan.json).
It maps every original AC of the 19 uncompleted implementation parents
TASK-71..78 and TASK-80..90 to **80 distinct TASK-109..188 child tasks**.
TASK-79 was already Done and is unchanged; TASK-91 remains the independent
release gate; TASK-92 is post-v3 and not a release-blocking implementation.

- Keep all original parent AC and their feature-richness floor. Parent
  dependencies on children are **closure-only**; each child depends on
  upstream *completed* parent prerequisites or its immediately preceding
  sibling, never on itself or its own parent.
- Each child fixes a finite named syntax/semantic variant set, two stable
  initial fixture IDs, an independently stated sample source/result/state
  oracle and explicit fail-closed boundaries. These are **planning constraints**,
  not a false claim that all typed expected domains and every dialect oracle
  are already implemented or certified.
- **Before implementation** freeze an individual child's full machine-readable
  acceptance inventory, including exact source/terminal bags, typed bounds and
  NULL, SQL TRUE and NOT TRUE directions, physical identities, adapters and
  all parser/dialect cells. Test and sign off **every** mandatory cell; justify
  any conditional cell with evidence, named owner and reviewed disposition.
  No blanket `where supported`, green-only signoff or implicit parent closure.
- TASK-71 starts at TASK-109. Each other implementation parent likewise
  resolves to its first dependency-ready child. Once all child PRs are merged,
  the parent receives an **independent six/nine/eight-AC closure review**
  before it can be marked Done and unblock TASK-91.

The original task acceptance criteria, the approved TASK-66 matrix, the
project-wide outcome-domain Definition of Done and any broader release
integration gates remain binding. Splitting schedules implementation; it
does **not** downgrade mandatory semantics or declare vendor-specific behavior
supported without evidence.
