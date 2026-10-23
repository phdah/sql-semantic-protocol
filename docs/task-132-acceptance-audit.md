# TASK-132 acceptance audit

The active, versioned matrix is [task-132-acceptance-freeze.json](task-132-acceptance-freeze.json). Each original TASK-75 acceptance criterion remains independent and unresolved until the merged child evidence is reviewed. This document describes the release-facing **current state**, not a changelog.

## Bounded mandatory cases

| Frozen case | SQL form | Canonical physical proof | Independent execution | Adapter parity | Disposition |
| --- | --- | --- | --- | --- | --- |
| 75-C132-P-direct_composite | EXISTS with two integer equality correlations | `physical_finite_exists_bag_plan` | Complete DuckDB source and terminal bags | Pending dbt/ODCS parity | Open |
| 75-C132-N-direct_composite | NOT EXISTS and NULL/duplicate absence | `physical_finite_exists_bag_plan` | Complete DuckDB bags | Pending dbt/ODCS parity | Open |
| 75-C132-P-grouped_composite | EXISTS through nonempty physical GROUP BY | `physical_finite_exists_bag_plan` with scoped `grouped_exists_equivalent` | Complete DuckDB bags | Pending dbt/ODCS parity | Open |
| 75-C132-N-grouped_composite | NOT EXISTS after grouped inner | Same exact bag oracle, no HAVING | Complete DuckDB bags | Pending dbt/ODCS parity | Open |
| 75-C132-P-joined_composite | EXISTS through a two-key inner JOIN | `physical_finite_joined_exists_bag_plan` | Complete three-source DuckDB bags and identity pairs | Pending dbt/ODCS parity | Open |
| 75-C132-N-joined_composite | NOT EXISTS after joined inner | Same exact model with full Cartesian absence | Complete three-source DuckDB bags | Pending dbt/ODCS parity | Open |
| 75-C132-P-dbt-subquery-orders | Committed multi-producer `subquery_orders.sql` | Independent source-to-producer DAG proof not yet established | Compiled dbt/DuckDB source, intermediate and final terminal snapshots | dbt execution present; ODCS proof parity pending | **Blocking** |

**Source identity:** The joined oracle stores complete `[inner_row_id, joined_row_id]` pairs, never independently assembled per-key or guessed physical rows. The outer SELECT retains duplicates and enumerated numeric terminal values, NULL status and inclusive bounds. SQL UNKNOWN is NOT TRUE in `WHERE` and can never establish a qualifying join or correlated equality.

**Fail-closed boundaries:** Non-INNER joins, unsupported predicate trees, absent/ambiguous source aliases, extra group shaping (HAVING), OFFSET, non-column/nonnumeric equality and duplicate physical IDs are not Exact. Native non-DuckDB engine/session attestations are conditional TASK-132 responsibility, not assumed from shared parser acceptance.

## Acceptance criteria closure

1. **AC #1, frozen executable inventory:** seven named mandatory cases, 13 dialects each, three adapter paths each; physical instances and source/terminal bags captured. The matrix still has unverified adapter/status cells. **Open.**
2. **AC #2, canonical physical completeness:** direct, grouped and joined bounded equality proof paths implemented and guarded. The composed multi-producer dbt case lacks an end-to-end canonical physical proof. **Open.**
3. **AC #3, native executable oracle:** independent DuckDB direct/joined/grouped source/terminal snapshots cover six cases; compiled dbt oracle enumerates input, producer and terminal bags for the seventh. Identity, negative and failure behavior require final-head review. **Open pending verification.**
4. **AC #4, 13 dialects, dbt/ODCS and vendor/session boundaries:** all 13 direct/grouped/joined native parser paths are exercised by public tests, but mandatory adapter-equivalent source proof and classification matrix is unfinished. Vendor behavior outside executable DuckDB remains uncertified. **Open.**
5. **AC #5, final verification/signoff:** no signoff until the final head passes format, strict lint, full test matrix, no-default-features, docs, dbt e2e and snapshots; independent two-axis review and parent AC reconciliation must follow. **Open.**

## Parent TASK-75

Parent original AC #1 through #6 require separate reconciliation of TASK-129, 130, 131 and this TASK-132 against all original families (EXISTS/IN/quantified/scalar, correlation, LATERAL/APPLY, NULL/duplicates, lineage and adapters). Green child CI is necessary but not sufficient. TASK-75 remains **To Do**, and TASK-91 remains a separate release gate.
