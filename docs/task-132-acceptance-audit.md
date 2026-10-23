# TASK-132 bounded acceptance signoff

The versioned mandatory contract is [task-132-acceptance-freeze.json](task-132-acceptance-freeze.json); the public machine-readable inventory is [coverage-manifest.json](coverage-manifest.json). All seven mandatory fixture cases are accompanied by complete source and terminal bags, typed domains, positive and NOT TRUE source-row identities, and 13 explicit parser paths with three representable adapter paths per case.

## Complete verified mandatory matrix

| Mandatory case | Canonical physical evidence | Independent oracle and adapter parity | Classification |
| --- | --- | --- | --- |
| 75-C132-P-direct_composite | Complete composite-key EXISTS equality; matching inner source IDs | Public Rust integration across 13 dialects, DuckDB full input/output, SQL/dbt/ODCS parity | Exact |
| 75-C132-N-direct_composite | Complete composite-key NOT EXISTS; NULL/duplicate/absence | Same three independent adapter paths and DuckDB oracle | Exact |
| 75-C132-P-grouped_composite | Nonempty local GROUP BY preserves Boolean EXISTS emptiness; physical candidate multiplicity is not inferred | Full source/terminal DuckDB bags and 13 dialect + three adapter proofs | Exact |
| 75-C132-N-grouped_composite | NOT EXISTS requires the complete grouped candidate absence | Same independent and adapter oracles | Exact |
| 75-C132-P-joined_composite | One nested INNER JOIN, two correlated keys, every qualifying inner/joined physical pair | Complete three-source DuckDB row-identity bag and 13 dialect + three adapter proofs | Exact |
| 75-C132-N-joined_composite | One nested INNER JOIN, SQL UNKNOWN and complete anti-membership absence | Same native full bag and adapter oracles | Exact |
| 75-C132-P-dbt-subquery-orders | All raw source rows recursively generate stg_orders, stg_customers, enriched_orders and correlated subquery_orders through the canonical graph | Actual compiled dbt/DuckDB full source/intermediate/terminal snapshots; independent identical exact output from direct compiled CREATE VIEW SQL and ODCS source schema; compiled SELECT parsing for all 13 exposed dialects | Exact |

The seven required cases account for **91 classified parser cells** and **21 canonical SQL/dbt/ODCS adapter proof cells**, all verified. Only DuckDB runtime SQL execution is certified; the other 12 named parser dialects are parser/canonical analysis compatibility, not assertions of native vendor runtime, collation, timezone or transaction behavior.

## Independent physical obligations

The direct and grouped solver preserves each qualifying/rejected outer physical identity, the complete multiset of matched inner IDs (including duplicates), typed enumerated integer outputs, SQL NULL and inclusive bounds. The joined solver independently evaluates one joint three-source assignment, preserving every `[inner_id, joined_id]` pair and full outer SELECT multiplicity; it never multiplies a surviving outer row by the number of inner witnesses.

For the named producer case, `physical_finite_producer_exists_plan` starts only from schema-validated raw source bags; traverses canonical named producer edges in dependency order; evaluates WHERE, projection, CASE, INNER JOIN and nested correlated EXISTS; and returns per-layer full bags, original physical leaf ID provenance, rejected inputs, terminal rows and exact scalar domains. Source row 2 and customer 20 are rejected at their staging layers. The complete final physical terminal bag is `[(amount=42, region="north")]`; amount has precisely `[42,42]` inclusive and region precisely `{"north"}`, with no NULL. This is checked against a separate DuckDB dbt run, not used as the engine implementation.

Source timestamps lacking timezone metadata are Residual until a caller explicitly attests the physical zone. The dbt conformance test checks DuckDB `typeof(created_at) = TIMESTAMP` and attests its actual no-zone behavior; a contradictory or non-timestamp attestation is rejected. A caller must also attest binary text collation/no padding for executed text comparisons. Duplicate physical source identity or schema contradiction is Impossible; unsupported predicates, unknown datatypes, alias resolution, unsupported row shapes, HAVING/OFFSET or vendor rules stay Residual rather than guessed Exact.

## Acceptance criteria review

1. **AC #1, freeze and inventory:** Both mandatory IDs, every named variant, all seven SQL/physical/terminal bags, typed bounds, source aliases, 91 parser cells, 21 adapter cells, TRUE/NOT TRUE and conditional owner/disposition are recorded in the versioned task-owned freeze and coverage manifest.
2. **AC #2, exact canonical realization:** Complete physical source truth and row multiplicity are independently proven for direct, grouped, joined and multi-producer SQL. No generator-specific SQL reparsing or invented intermediate snapshots.
3. **AC #3, executable native oracle:** All six compositional fixture forms execute against independent DuckDB physical bags, including NULL/duplicate/rejected/negative cases. The seventh executes real dbt-compiled production and rejects altered/contradictory physical metadata.
4. **AC #4, adapters/dialects/boundaries:** All 13 exposed dialect names are exercised for the shared parser forms and compiled dbt models; direct SQL, dbt artifact and equivalent ODCS schema evidence yield identical canonical physical truth. Native non-DuckDB runtime/session claims remain explicitly conditional and unapproved.
5. **AC #5, final-head review:** Require a green final-head Rust format, strict Clippy, full/default and no-default tests, docs, dependencies and dbt E2E run; independent standards/spec inspection must match this freeze before marking the child Done.

## Two-axis implementation review

**Standards** (fixed point `main`; `AGENTS.md`, `CONTRIBUTING.md`, `docs/code-map.md` and Fowler smell checks): Canonical AST normalization remains in analysis, the producer evaluation is a distinct focused physical module, and source metadata attestations remain in `AnalysisBundle`. No SQL parser, dbt engine dependency, filesystem access, unsafe Rust, hardcoded fixture identities or duplicated comparison-law flag enters the physical proof. Source/schema qualification and duplicate projected labels fail closed. All public exports have library entrypoint documentation and the code map names each owner. Remaining native warehouse text/session differences are explicit attestations or Residual, not silent defaults. No standards finding remains that blocks acceptance; strict lint/format/docs are separate CI gates.

**Spec** (TASK-132 AC #1–#5, frozen seven-case version 2, and original TASK-75 AC #1–#6): The exact mandatory core is exercised at its public API seams, including both membership directions, source multiplicity, NULL/duplicates, typed terminal domains and closed-world rejected populations. The named compiled dbt multi-producer fixture has a physically constructed terminal proof and independent DuckDB output oracle, with full direct SQL/ODCS parity and the explicit 13 dialect parser inventory. Original parent TASK-75 remains open for separate sibling reconciliation; TASK-91 release scope has not been pulled into this child. No mandatory variant is excused by a Residual or native-vendor assumption. No remaining spec-level implementation gap is identified.

## Original TASK-75 mapping and release

TASK-132 contributes evidence to all original TASK-75 criteria #1–#6, but does not close the parent independently. TASK-75 requires an independent reconciliation across the merged TASK-129, TASK-130, TASK-131 and TASK-132 child evidence after this PR lands. Conditional native-vendor/session behavior has explicit TASK-132 ownership and a parser-only/no-runtime-certification disposition. TASK-91 and protocol 3.0.0 release approval remain separate gates. No child success grants a release claim.
