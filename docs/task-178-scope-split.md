# TASK-178: maintainer-approved bounded scope split (2026-11-10)

**Decision:** Complete and merge PR [#197](https://github.com/phdah/sql-semantic-protocol/pull/197) as the verified **portable canonical-equivalence baseline**, not as certification of the full 163 × 13 mandatory semantic matrix. The original TASK-178 requirements are *reassigned*, not waived, and TASK-88/TASK-91 stay blocked. This supersedes the pre-split "do not merge" statements in `docs/task-178-acceptance-audit.md` for **this bounded baseline only**, not for global semantic certification.

The maintainer explicitly requested smaller tasks before merging. The old initial seven-case v1 freeze preceded the later 163-case observation inventory; this history cannot be changed retroactively. The full observation inventory is **not** an independent typed/domain or physical proof.

## Original five TASK-178 acceptance criteria (historical, preserved verbatim)

- [ ] #1 Before implementing, freeze all named variants and fixture IDs in a task-owned versioned acceptance matrix linked from `docs/coverage-manifest.json`: SQL shape, dialect/parser equivalence, typed output domains (NULL/inclusive-exclusive), exact source and terminal snapshots, TRUE/NOT TRUE/Impossible/Residual proof, source identities, adapter and external-engine evidence. Unverified mandatory cells block Done.
- [ ] #2 Implement only the finite listed mandatory families and their stated oracle behavior, preserving strongest sound value domains, cross-source correlation, whole-bag multiplicity, complete downstream outputs and deterministic canonical proof. Do not infer independent witness conjunction, silently discard data, or rely on sql-tdg reparsing.
- [ ] #3 Add executable, public-API DuckDB differential cases for positive and rejected or impossible directions, including fixed NULL/duplicate or counterexample scenarios. Compare complete physical inputs, emitted typed protocol claims and full terminal bags/replay artifacts, not only parsing or generated expectations.
- [ ] #4 Check the 13 project dialect parser/equivalence cases where meaningful and direct SQL/dbt/ODCS parity for available facts. Record named vendor/session or external consumer limitations, exact evidence required, stable Residual/Impossible classification and explicit review disposition. No unexecuted native-engine claim.
- [ ] #5 Verify final-head fmt, strict Clippy, full Rust tests/docs/no-default-features, dbt E2E, active schema/golden and relevant pinned sql-tdg tests. Independently sign off each frozen mandatory cell and its mapped parent AC before Done/merge; TASK-88 stays open until all children and original AC pass.

## Ownership without omissions

The exact frozen [163-case inventory](task-178-full-variant-inventory.json), the original TASK-177 SQL fixtures, and 13 parser dialect identities remain the source of truth. [Machine-readable ownership](task-178-scope-split.json) maps **all 11 variant families and all 163 cases** exactly once. Every new ticket inherits the outcome-first Definition of Done in `AGENTS.md` and original TASK-88 AC #1..#8.

| Queue | Owner | Original variant families | Named cases | Dialect cells | Delivery |
| --- | --- | --- | ---: | ---: | --- |
| 1 | TASK-178 | Versioned observation inventory, portable normalization, representative DuckDB oracles, conservative failures and exact-head consumer verification | Starter 7; whole observed 163 | 2,119 observations, **0 full-cell certifications** | PR #197 bounded baseline |
| 2 | TASK-192 | Predicates and structural types | 43 | 559 | Strongest sound typed bounds, three-valued membership and independent source/terminal negatives |
| 3 | TASK-193 | Joins and correlated sources | 24 | 312 | Shared source identity, NULL extensions, aliases, joined bags and negative witnesses |
| 4 | TASK-194 | Set operations | 22 | 286 | Tuple/NULL multiset semantics, nested branches and column alignment |
| 5 | TASK-195 | Grouping, windows and ordering | 36 | 468 | Empty grouping, rank/frames, QUALIFY, peer/tie pagination and sampling boundaries |
| 6 | TASK-196 | Schema, DML and DDL | 38 | 494 | Typed schema/constraint facts, complete before/after mutation state, produced relations |
| 7 | TASK-197 | Cross-family SQL/dbt/ODCS parity | All 163 reused | All applicable | Certified representable adapter facts, explicit nonrepresentable boundaries |
| 8 | TASK-179 | Vendor and session execution laws | Conditional cases | Per owner | Engine-specific assumptions and actual native attestations |
| 9 | TASK-191 | Parser/Unsupported/native rejected cases | 55 parser rejects / 116 Unsupported / 23 native exceptions (overlapping registers) | Per owner | Reviewed fail-closed adjudications, no false equivalence |
| 10 | TASK-180 | Independent complete-matrix certification | All original | All 2,119 | Independently sign off source, typed/NULL, positive/NOT TRUE/Impossible/Residual, adapter and vendor dispositions |
| Parent | TASK-88 | Original parent #1..#8 | All original | All mandatory | Remains To Do until every child merges; TASK-91 then controls release |

The categorical case counts **43 + 24 + 22 + 36 + 38 = 163**. The 2,119 observed dialect cells are **not** automatically made Exact by the split. TASK-179, TASK-191 and TASK-180 are existing tasks with distinct ownership; they are not duplicated by the new child tasks.

## Original TASK-178 acceptance handoff

- **Original #1 (complete pre-implementation matrix):** TASK-192..196 own case-specific expected types/source/terminal/proof ledgers, TASK-180 owns independent whole-matrix signoff. The historical seven-case pre-implementation deviation is recorded permanently, not falsely fixed.
- **Original #2 (all strongest sound semantic outcomes):** TASK-192..196 own the complete 163-case semantic and source-correlation proof, with cross-operator tests where applicable, TASK-180 signs off completeness.
- **Original #3 (independent positive/negative native oracles):** TASK-192..196 own executable DuckDB and rejected/Impossible evidence for each relevant case, TASK-180 checks no omission.
- **Original #4 (13 dialects, adapters and vendor/session boundaries):** TASK-192..196 own parser/shared AST facts, TASK-197 audits all representable SQL/dbt/ODCS facts, TASK-179 handles vendor/session laws, TASK-191 parser/unsupported/native boundary adjudications, TASK-180 final review.
- **Original #5 (final CI/consumer and independent signoff):** PR #197 verifies its own bounded changes with final-head CI and pinned sql-tdg; every follow-up repeats applicable final-head CI and consumer gates. TASK-180 requires a distinct independent reviewer for the complete matrix and each original parent acceptance criterion.

## PR #197 bounded acceptance evidence

- Frozen seven-case `docs/task-178-acceptance-freeze.json` and immutable TASK-177 parser snapshots, with additive 163-case/13-dialect *observation* inventory.
- Canonical parser-boundary normalization for ROLLUP/CUBE empty-source groups, unsupported STRUCT CAST diagnostic forms, and TOP/FETCH/LIMIT selected row-shaping, including meaning-preserving synonyms and explicit WITH TIES/MSSQL LATERAL boundaries.
- Independent DuckDB physical positive/negative source and terminal evidence for representative families, 53 relational terminal/domain-containment checks and 78 direct/dbt/ODCS family variants over 13 parser dialects. Typed containment does not certify strongest possible domain; DuckDB execution does not certify other engines.
- Exact pre-split semantic head `3971b2bc23ee3face8d6a3ac6a4bfe96f40cf2d8` passed [9/9 protocol CI](https://github.com/phdah/sql-semantic-protocol/actions/runs/42882374317) and its pinned [sql-tdg consumer CI 5/5](https://github.com/phdah/sql-tdg/actions/runs/42882183376). The final scope-split-only commit still requires its own head CI and consumer re-pin before declaring final-head verification.
- No `release_approved`, global source/terminal/typed certification or TASK-88/TASK-91 closure is implied by marking the **bounded TASK-178** Done.

## Delivery order

`TASK-178 (this PR)` → `192` → `193` → `194` → `195` → `196` → `197` → `179` → `191` → `180` → independent TASK-88 parent acceptance → TASK-91 release gate.

This dependency chain is acyclic: **no child depends on TASK-88**.
