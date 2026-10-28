# TASK-148 bounded acceptance audit

Owner: TASK-148, child 4/4 of TASK-80. Reviewed freeze: `docs/task-148-acceptance-freeze.json` v2. Initial freeze is retained at `docs/task-148-acceptance-freeze-v1.json`. The v2 review adds the missing NULL/duplicate/row-identity and per-dialect evidence demanded by the **original**, unchanged acceptance criteria; it does not downgrade any exact mandatory variant or reclassify omitted mandatory implementation as a conditional residual.

## Exact and negative evidence

| Frozen variant | Independent evidence | Result |
| --- | --- | --- |
| 80-C148-P VALUES -> SELECT count -> CTAS | `task_148_ordered_insert`: complete before/after bag, count 2, native DuckDB report | Exact |
| 80-C148-P SELECT with column reorder -> CTAS | Same; source TRUE and NOT TRUE rows, untouched target row | Exact |
| 80-C148-P local CTE -> INSERT -> CTAS | Same; complete intermediate, source and terminal rows, separate DuckDB stage query | Exact |
| 80-C148-P DEFAULT VALUES -> CTAS | Same; authoritative per-column scalar defaults, typed bounds | Exact |
| 80-C148-N duplicate PK across statements | Same; impossible second INSERT, first statement remains committed in native DuckDB | Impossible |
| 80-C148-N BEGIN/COMMIT | Same; unsupported transaction is explicit; no rollback/isolation law invented | Residual |
| NULL predicate false/unknown source rows | `task_148_negative_matrix`: SQL IS NOT TRUE and full DuckDB bag | Exact with two rejected rows |
| Duplicate source bag | Same; source multiplicity, resulting target multiplicity and exact domain frequencies | Exact |
| Duplicate enforced key *within* one INSERT SELECT | Same; contradiction and DuckDB atomic error with unchanged target | Impossible |
| Missing closed-world source snapshot | Same; explicit missing-population gap | Residual |
| INSERT SELECT reading a target just written | Same; two target transitions, complete source/target snapshots and DuckDB output | Exact |

Each positive case attests a full physical source/target bag rather than a sample. The source row ordinal and target row ordinal are owned by the fixture; SQL TRUE/NOT TRUE rows are independent DuckDB observations and map back to those identities. Bound inclusivity and exact value frequencies are asserted through the public state/schema API. Identity and generated-column allocation are not fabricated from parser support.

## Acceptance criteria closure

- **AC #1:** Six originally frozen main cells, five contrast cells and the stable `80-C148-P/N` IDs have named SQL, source/target bags, constraints, typed value domains or a justified unknown, proof classification and fixture-owned canonical row IDs. V2 retains v1 and explicitly maps all TASK-80 original AC #1..#6, while the parent remains unsigned.
- **AC #2:** Sequential writes use the existing parser-independent `physical_ordered_write_state_plan`; tests require each intervening target state and final bag, including self-source reads, rejected duplicate keys and missing source evidence. No new parser-dialect or adapter-specific semantic engine was introduced.
- **AC #3:** The four original INSERT families and negative contrasts are backed by independent DuckDB source, intermediate and terminal snapshots, SQL TRUE/NOT TRUE, two-row SELECT count, duplicate populations, unknown predicates and atomic constraint failure.
- **AC #4:** The 13 dialect names classify every SQL statement in every one of the six original cells through 78 frozen case-by-dialect statuses (182 statement cells). Another 52 shared-family cells and 39 vendor-modifier cells enforce fail-closed boundaries. Direct SQL, dbt-compiled catalog and ODCS equivalent schema evidence are compared on two-append, CTE, defaults, missing-source and PK-contradiction cases.
- **AC #5:** Final-head `fmt`, strict Clippy, full and no-default-features Rust tests/docs, dbt Core E2E and protocol/schema snapshots must all pass on the **same** head. See the CI evidence section below; do not check this criterion without final-head verification.

## Explicit conditional boundaries

- **INSERT OVERWRITE and native conflict modifiers:** parser acceptance is not an executed replacement scope, error-precedence or engine/session law. Uncertified syntax is Unsupported/Residual, with TASK-148 evidence owner; no portable Exact promotion. Vendor-specific extensions require an independently attested follow-up. Native engine execution beyond DuckDB is **not** certified.
- **dbt run semantics:** compiled manifest/catalog provides SQL and schema facts. It does not prove chronological execution of user-specified DML. Parity tests call the same canonical planner using equivalent evidence, and repo dbt Core E2E exercises the supported compilation/materialization path; no unexecuted model is advertised as an actual write.
- **Transaction BEGIN/COMMIT, rollback, native concurrency and isolation:** unsupported program statements cannot silently disappear. These are explicit Residual proofs; transaction execution belongs to TASK-84, not a false exact TASK-148 signoff.

These conditional cases have been reviewed for bounded portable child scope. **They are not release approval**, and TASK-91 retains independent release authorization.

## Review

**Standards axis:** Uses single canonical physical-state owner, no dependencies, no `unsafe`, deterministic typed domain assertions, parser/analysis/emission separation, and versioned executable acceptance cases. Structural owner index updated in `docs/code-map.md`.

**Specification axis:** All named four INSERT families, preceding CTE, later SELECT/DDL, full physical source-target states, duplicate/NULL/constraint safety and 13-dialect plus adapter parity have finite tests. Unsupported native/session laws remain correctly unproved.

## Final-head CI evidence

Pending final-head CI at the last implementation commit. Do not mark TASK-148 Done until all tests and both reviews pass. TASK-80 remains open for its original independent parent acceptance verification.
