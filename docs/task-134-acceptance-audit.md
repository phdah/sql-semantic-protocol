# TASK-134 independent acceptance audit

**Scope:** TASK-134, child 2/4 of TASK-76. TASK-76 and the v3 release gate retain independent approval requirements. This audit is not a claim of native non-DuckDB SQL execution.

## Bounded contract

The v1 task-owned acceptance inventory is `docs/task-134-acceptance-freeze.json`.
It fixes the positive `76-C134-P` and nonmembership `76-C134-N` families, six mandatory cases, three reviewed conditional cases, complete physical source values, SQL, expected terminal multiplicities, source identities, per-column exact finite domains with inclusive bounds, and explicit NULL rules.

The frozen 117-cell dialect matrix contains **78 parser/canonical Exact cells** for six mandatory cases and **39 Residual cells** for three conditionals. The observed results came from the full `task_134_frozen_matrix.rs` execution in CI run 40227228471; every named dialect accepted the frozen SQL as parsed syntax. This does not certify execution on those 13 production engines.

The 27 adapter cells are covered by `task_134_adapter_parity.rs`: direct SQL, compiled dbt manifest with declared source datatypes, and ODCS source-schema evidence. Mandatory cells require identical closed-world terminal bags, contributor identities and domains; conditional cells require typed fail-closed behavior. The executed dbt Core fixture `task134_distinct.sql` and seed `task134_source.csv` independently prove physical source [1,1,2,3] and final output [1,2,3].

## Proof and fail-closed law

- `WITH TIES`: source [3,2,2,1], descending FETCH 2, complete terminal [3,2,2], selected three physical rows, rejected one, exact domain inclusive [2,3]. An independent DuckDB RANK expression verifies the native full terminal bag.
- `DISTINCT`: duplicate contributors remain individually identified even though equal physical values collapse into one terminal tuple. The physical proof certifies numeric and NULL equality only with authoritative signed-integer source evidence.
- `DISTINCT ON`: an identical projected tuple for every equal key has exact terminal-bag membership without fabricating a row winner. Non-key outputs require an attested deterministic leading-key sort. Ambiguous tie survivors produce Residual.
- `NULL ORDER`: explicit NULLS FIRST/LAST has a certified physical sort law. Implicit ordering with observed NULL and unbounded or uncertified order stays Residual.
- `QUALIFY` and other producer layers: no source identity shortcuts; unproved operator composition remains Residual. This child does not certify general pagination after joins or groups.
- Exact/Impossible/Residual: a complete attested source yields exact terminal and inclusive value domains, requested absent terminal tuples yield Impossible, unsupported order or equality laws yield Residual. An impossible source schema is independently handled by the canonical physical validation.

## Original acceptance mapping

| TASK-134 criterion | Evidence | TASK-76 original |
| --- | --- | --- |
| #1 freeze exact and conditional cases | task-owned inventory, `docs/coverage-manifest.json` | #1, #2, #3, #5, #6 |
| #2 canonical exact domain and full physical proof | `src/protocol/aggregation.rs`, `src/analysis/sets.rs`, `src/physical_realization/page_bags.rs` | #1, #2, #3 |
| #3 independent source/terminal oracle | `tests/task_134_{with_ties,distinct_on,frozen_matrix}.rs` | #1, #2, #5 |
| #4 parser and adapter parity | `tests/task_134_{frozen_matrix,adapter_parity,dbt_core_e2e}.rs` | #2, #3, #6 |
| #5 independent standards/spec/CI signoff | final-head Cargo formatting, strict lint, full tests and dbt end-to-end | #5, #6 |

## Standards and scope review

Canonical AST conversion stays in analysis, typed DISTINCT grouping is reused from the existing public `Aggregation` domain model, and the physical executor does not reparse SQL. The JSON contract adds an optional `with_ties` field only for direct pages; historical unshaped/non-tie page documents remain unchanged. The new `terminal_contributors` Rust field is a public API change requiring the next major SemVer release; do not claim prior API-level backward compatibility.

The conservative integer equality guard avoids assuming vendor text collation or decimal coercion. Stable physical identities and negative membership are retained for all complete source rows. `AGENTS.md`, `CONTRIBUTING.md` and Fowler smell review identify no need for a consumer-specific contract or a new dependency.

## Final CI and disposition

**Pending final-head verification.** Do not mark TASK-134 Done until mandatory and conditional SQL, dbt, ODCS, schema/golden, docs, formatting and strict lint jobs all pass on the final PR head. Do not merge or mark TASK-76 complete based on this child audit.
