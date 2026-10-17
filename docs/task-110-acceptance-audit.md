# TASK-110 bounded acceptance signoff

Parent: TASK-71 (not Done). Frozen versioned matrix: [task-110-acceptance-freeze.json](task-110-acceptance-freeze.json).
Delivery: https://github.com/phdah/sql-semantic-protocol/pull/119

## Independent case-by-case proofs

| ID / family | Frozen complete input bag | Full terminal-output identities | Oracle/domain | Certified constructive proof |
| --- | --- | --- | --- | --- |
| 71-C110-P-CHAIN / three INNER inputs | L={l1:1,l2:1}, R={r1:1,r2:1}, T={t1:1} | l1:r1:t1, l1:r2:t1, l2:r1:t1, l2:r2:t1 | 4 output rows; key [1,1] inclusive, non-NULL; NOT TRUE bag empty | JoinChainPopulation: single common matched key=1; 2x2x1; 3 closed physical sources |
| 71-C110-P-SELF / aliases a,b | L={l1:1,l2:1}, two SQL aliases of L | l1:l1, l1:l2, l2:l1, l2:l2 | 4 output rows; key [1,1] inclusive, non-NULL | JoinPopulation CommonMatched 2x2, exactly 1 closed physical bag |
| 71-C110-P-SHARED / stage, mart | both stage and mart are identity-preserving copies of L; R={r1:1} | l1:r1:l1, l1:r1:l2, l2:r1:l1, l2:r1:l2 | 4 output rows; key [1,1] inclusive, non-NULL | JoinChainPopulation 2x1x2, exactly 2 unique physical closed bags L,R |
| 71-C110-N-NULL / rejected | L={l1:1,l2:NULL}, R={r1:1,r2:2}, T={t1:2} | No matching output rows; all four Cartesian tuples are NOT TRUE | 0 terminal rows on the frozen closed bag, output domain empty; this is not a universal impossibility | Exact DuckDB physical negative/closed-world oracle; constructive existential for other source assignments is not confused with this fixed database |

All physical bags carry stable row identities. The positive and rejected bags are executed independently by DuckDB in `tests/task_110_join_chains.rs`. Both join conditions are evaluated over the **same** physical population; independently sufficient pairwise proofs are not promoted to a chain proof. The chosen witness key 1 is attested as representable by every physical signed-integer key column. NULL never equals NULL under ordinary SQL `=`.

## Acceptance criteria

1. **Freeze:** 2 fixture identities, 4 mandatory concrete variants and all 52 parser dialect cells, source instance mappings, complete source bags, typed outcome domains, expected full row-identity bags, adapter classifications, and conditional boundaries. Versioned freeze linked in `docs/coverage-manifest.json`.
2. **Constructive model:** canonical `JoinChainPopulation` with three row instances, four join-column references, one certified matched integer value, a single checked multiplicity product, closed unique physical source bags, and canonical JSON schema. Direct aliased self-joins reuse a single physical source rather than independently counted alias inputs. Nonqualifying join shapes remain Residual.
3. **Independent execution:** `tests/task_110_join_chains.rs` executes DuckDB positive, self, shared-producer and NULL-rejected full bags with stable IDs, exact counts and value-domain extrema, including the fixed-bag closed-world empty terminal. No vendor certification is inferred for other engines.
4. **Dialects/adapters:** all 13 exposed dialects parsed through the public analyzer for the frozen forms; `postgres` alias maps to `postgresql`. Direct SQL plus real compiled dbt manifest and ODCS equivalent key metadata tested in `tests/task_110_join_adapters.rs` (feature-gated when ODCS disabled). Extra `SchemaSourceKind` catalog/manifest/external provenance equivalence. Only DuckDB oracle execution, other vendors are parser-equivalent, not execution-certified. Computed/non-equality/OR join keys and untransported identity/type evidence fail closed; TASK-113 owns non-equality/OR follow-on work.
5. **Verification:** focused Rust public API, canonical JSON schema, independent DuckDB, both feature configurations, full `cargo test`, rustfmt, strict Clippy, docs, and project dbt Core E2E are PR CI gates. Final merge requires all checks green **on the final PR head**. An acceptance audit cannot substitute for green CI.

## Release and handoff

TASK-110 is a bounded child, not full TASK-71 completion or v3 release approval. Maintain the parent in progress and do not start TASK-111 until this PR is merged into main.
