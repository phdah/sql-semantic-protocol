# TASK-122 final bounded acceptance audit

Date: 2026-10-19  
Owner: TASK-122; parent: TASK-73 (remains To Do)  
PR: https://github.com/phdah/sql-semantic-protocol/pull/132  
Frozen matrix: [task-122-acceptance-freeze.json](task-122-acceptance-freeze.json), version 1,
committed before semantic implementation.

## Reviewed cells

| Frozen cell | Entire physical bag and terminal oracle | Group-level negative proof |
| --- | --- | --- |
| 73-C122-P-distinct | Signed-integer group k=1 with [1,1,NULL]: COUNT(DISTINCT x)=1, COUNT(x)=2; exactly one terminal row | Group k=2 is FALSE in HAVING, retains its closed source row |
| 73-C122-P-filter | FILTER (WHERE x>1) counts one contributor in each group; NULL gives UNKNOWN and is excluded; aliases and second aggregate stay independent | Non-TRUE row-level FILTER contributors are excluded, without discarding physical source rows |
| 73-C122-N-unknown | No terminal rows | Two existing groups evaluate HAVING as UNKNOWN, with every physical source identity retained |
| 73-C122-P-boolean | k=1 and k=2 survive with their exact COUNT/SUM results | k=3 is FALSE after AND/OR with SQL NULL, retains its two-row complete bag |

The matrix explicitly specifies each full source bag, projected complete terminal bag,
group physical row identities, per-aggregate contributor identities, SQL truth, NULL membership,
exact finite output sets and inclusive integer extrema. The public API regression tests
independently execute each complete source and terminal bag in DuckDB, and compare
identities, duplicates, rejected groups and canonical terminal `ValueDomain` values.

## Acceptance mapping

1. **Frozen inventory:** the versioned task-owned positive/negative fixture matrix and
   `docs/coverage-manifest.json` register four executable cases with named dialects,
   adapter sources, expected typed outcomes, proof classifications and conditional owner.
2. **Canonical semantics:** `src/physical_realization/aggregate_bags.rs` evaluates
   normalized COUNT(DISTINCT x), FILTER and HAVING over one schema-attested complete
   physical bag. Exact surviving output domains and FALSE/UNKNOWN rejected groups
   derive from that same bag. Contradictory source evidence stays Impossible; absent
   or unsupported scalar evidence stays Residual.
3. **Independent source/terminal execution:** `tests/task_122_aggregate_having.rs`
   verifies DuckDB full source readback, full terminal bag, exact output domains,
   group TRUE/FALSE/UNKNOWN and every contributor identity, including rejected groups.
4. **Dialect/adapter parity:** the 13 listed parser dialects are exercised for the
   shared forms. FILTER parser rejection, if any, is a conditional grammar boundary,
   never certified as native-engine equivalence. The public
   `tests/task_122_aggregate_adapters.rs` executes direct SQL, compiled dbt
   and ODCS-attested source parity for all four cells. Equivalent evidence
   yields the same canonical terminal bag, output domains and groups.
5. **Verification:** 9/9 GitHub Actions jobs passed for semantic head
   `62642b51a763f8fcc0e7db63df13768a8364e545`:
   [run 39275972342](https://github.com/phdah/sql-semantic-protocol/actions/runs/39275972342).
   Jobs include Rust formatting, strict lint, tests, docs, no-default-features,
   dependency checks and dbt E2E. Final metadata-head CI remains a pre-merge gate.

## Reviewed limits and disposition

COUNT(DISTINCT x) is certified on typed signed integers, including duplicate and
NULL behavior. DISTINCT SUM, computed DISTINCT arguments, unproved text comparison,
decimal/rounding, unknown scalar functions and unsupported grouping constructs
remain explicit Residual, not silent Exact or Impossible. The oracle certifies
DuckDB execution; other native vendor/session execution is not claimed.
TASK-123 owns filtered JOIN/GROUP composition and grouping variants. TASK-124
and the parent TASK-73 retain their independent full acceptance and release gates.
No native-vendor certification or release approval is inferred from this child.
