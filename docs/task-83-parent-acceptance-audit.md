# TASK-83 independent parent acceptance audit

Scope: independently reconcile the six original TASK-83 acceptance criteria with merged TASK-157..160 work. This is not a TASK-91 release approval.

## Merged prerequisites

| Child | Merged implementation | Acceptance freeze | Executable evidence |
| --- | --- | --- | --- |
| TASK-157 | [#178](https://github.com/phdah/sql-semantic-protocol/pull/178) | docs/task-157-acceptance-freeze.json v2 (9 cases) | tests/task_157_state_oracles.rs, tests/task_157_adapter_parity.rs |
| TASK-158 | [#179](https://github.com/phdah/sql-semantic-protocol/pull/179) | docs/task-158-acceptance-freeze.json v4 (8 cases) | tests/task_158_ordered_alter.rs, tests/task_158_adapter_parity.rs |
| TASK-159 | [#180](https://github.com/phdah/sql-semantic-protocol/pull/180) | docs/task-159-acceptance-freeze.json v3 (18 cases) | tests/task_159_lifecycle.rs, tests/task_159_dialect_matrix.rs |
| TASK-160 | [#181](https://github.com/phdah/sql-semantic-protocol/pull/181) | docs/task-160-acceptance-freeze.json v3 (7 cases) | tests/task_160_ordered_lifecycle.rs, tests/task_160_adapter_parity.rs |

All four children are Done and merged on main. Their 42 frozen cases retain their own classifications, review histories, dialect and adapter boundaries. Inventory counts are not evidence of execution.

## Original acceptance criterion crosswalk

| Parent AC | Independent reconciliation |
| --- | --- |
| #1 CREATE/REPLACE, VIEW/MATERIALIZED VIEW, TEMP/TRANSIENT, LIKE/CLONE and qualified definitions | TASK-157 proves bounded CREATE/CTAS/replacement, logical/snapshot identities and qualified names. TASK-159 classifies vendor copying and session forms; absent native execution evidence remains Conditional/Residual. |
| #2 DROP/TRUNCATE/ALTER/RENAME and state effects | TASK-158 proves exact column transformations. TASK-159 checks destructive and renaming DDL. TASK-160 and the new parent oracle check ordered schema/data coexistence and TRUNCATE after ALTER. |
| #3 Object identity, IF EXISTS and vendor transaction laws | TASK-157 separates logical views and stored snapshots; TASK-159 owns existence behavior. TASK-84/TASK-163 retain transaction/session ownership. Parser success cannot certify vendor runtime rules. |
| #4 Before/after catalogs, lineage, data loss and uncertainty | Child full-bag typed proofs plus parent replay cover four input occurrences including duplicates/NULL, filtered read TRUE and NOT TRUE evidence, empty after TRUNCATE and absent after DROP. |
| #5 Complete statement sequences and native/dialect oracles | Parent positive replay executes seven SQL statements against native DuckDB, with both read bags and live catalog. Negative replay proves SELECT after DROP Impossible in the canonical proof and native engine. Child dialect matrices cover parser identities, not thirteen native engines. |
| #6 Tests, adapters, protocol documentation/schema | Child matrices retain thirteen parser dialects, SQL/dbt/ODCS parity for representable facts, active protocol/schema checks. New parent tests independently assert the original AC mapping and 42 child fixture identities. Public contract is unchanged. |

## Parent-owned frozen fixtures

docs/task-83-parent-acceptance-freeze.json v1 was committed before the tests.

- 83-PARENT-LIFECYCLE-P: CREATE empty table, INSERT (1, NULL, 1, 3), ALTER ADD b DEFAULT 2, read WHERE a > 1, TRUNCATE, read the empty table, DROP. First read must select source ordinal 3 and reject 0,1,2; its typed INTEGER domains are a=[3,3], b=[2,2] with inclusive endpoints. Second read remains typed and empty; DROP removes the catalog.
- 83-PARENT-DROP-READ-N: A SELECT after a witnessed DROP is Impossible, not an empty Exact output or a fabricated source.

tests/task_83_parent_lifecycle.rs verifies both cases through the public analyzer/physical lifecycle interface and independent DuckDB execution.

## Conditional boundaries and gates

LIKE/CLONE copying, TEMP/TRANSIENT visibility, vendor materialized-view refresh and implicit DDL transaction laws require independent versioned native evidence. TASK-157/159 own the object-specific boundaries; TASK-84/163 own transactions. These conditions must never hide a missing mandatory portable Exact case.

**Outstanding gates:** all nine final PR-head CI jobs, the independent Standards/Spec parent review, an independent reviewer identity or explicit TASK-83-specific owner exception, and parent-owner approval of all six original AC. Child and TASK-82 exceptions do not transfer. Until these gates pass, TASK-83 remains In Progress with six unchecked original AC; TASK-161 stays blocked and TASK-91 is unchanged.
