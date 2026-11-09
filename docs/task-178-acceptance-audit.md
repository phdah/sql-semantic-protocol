# TASK-178 initial acceptance audit (2026-11-09)

**Status: partial implementation only, NOT SIGNED OFF.** This artifact records independently specified expected results for a finite representative baseline; it does not change the original TASK-178 acceptance criteria, claim the full task-specific freeze is complete, or release TASK-88/TASK-91.

## Matt `engineering/implement` scope and standards

The initial public seam is `analyze_sql`/`to_json` plus a native DuckDB source/terminal execution oracle. The code uses the original TASK-177 SQL fixture IDs and compares **full emitted documents** across the 13 advertised dialects after nulling only the `inputs[*].dialect` provenance field. It does **not** compare AST shapes or infer semantic correctness from the absence of errors.

The versioned v1 starter matrix is [task-178-acceptance-freeze.json](task-178-acceptance-freeze.json). Seven complete original SQL example families have explicit source and output/after-state bags and the canonical comparison obligation:

| Family | Original SQL fixture | Physical oracle | Status |
| --- | --- | --- | --- |
| Predicate | S006 | >= 10 includes 10 twice; FALSE 5, UNKNOWN NULL; inclusive lower bound | Initial test added |
| JOIN | S043 | 2 x 2 duplicated key matches = four id=1 rows; NOT EXISTS rejects unmatched 2 and NULL | Initial test added |
| Set | S066 | UNION ALL retains all eight nullable and duplicate rows | Initial test added |
| Window | S106 | ROW_NUMBER creates values 1..4 on four source occurrences | Initial test added |
| Pagination | S159 | LIMIT 2 OFFSET 1 keeps [1,2], excludes one 1 and NULL | Initial test added |
| DML | S194 | INSERT adds exactly one (1,10) occurrence to complete poststate bag | Initial test added |
| DDL | S212 | CTAS from customers preserves [1,1,3,NULL] in produced relation | Initial test added |

The tests pin exact literal bag expectations independently of protocol generation. Native execution is **DuckDB only**, no alternate vendor certification or cross-engine type law is inferred. Predicate proof asserts the public typed inclusive lower value-domain; source/schema and all other outcome domains require stronger task-specific evidence.

## Original child acceptance accounting

- **AC #1: OPEN.** Initial versioned matrix and manifest link exist, but the full named variant/typed/parser/session/adapter matrix is not yet frozen or reviewed.
- **AC #2: OPEN.** Seven first-party representative forms receive full public-document structural checks. No general semantic lowering, transportability or end-to-end witness-composition correctness is asserted for missing mandatory forms.
- **AC #3: OPEN.** Seven independent DuckDB source/output or poststate oracles and the predicate TRUE/FALSE/UNKNOWN and join anti-membership cases were added; full positive/rejected/impossible source and terminal snapshots for all mandatory family cells remain absent.
- **AC #4: OPEN.** 13 dialect parser/canonical comparisons cover the seven all-accepted original SQL cases, but native vendor/session and complete representable SQL/dbt/ODCS evidence remain unverified.
- **AC #5: OPEN.** Formatting, strict Clippy, full/no-default Rust tests, docs, dbt E2E, active snapshots, exact-head pinned sql-tdg and independent review remain final-head gates, not implied by this audit.

## Explicit retained blockers

TASK-177's 23 observed full-document drifts across ROLLUP/CUBE/GROUPING, correlated LATERAL and STRUCT CAST have not been resolved or signed off. TOP, FETCH FIRST WITH TIES and MINUS need individual parser/native and meaning-preserving synonym evidence; the previously observed DuckDB FETCH original-SQL rejection remains a boundary, never a proved dialect-equivalence claim. TASK-191 owns 55 parser-rejected dialect cells, 116 analyzer Unsupported cells and 23 original-native-unproved cases. TASK-179 owns vendor/session laws. TASK-180 owns independent whole-matrix certification. These owners **do not** waive TASK-178's own required proof obligations.

## Two-axis review scope

**Standards:** no library/CLI protocol schema modifications, new dependencies, lint suppressions or unchecked Exact claims; all verification is through public APIs and independent SQL execution. CI, formatting and compilation need real execution before a Standards PASS can be given.

**Spec:** the new tests exercise representative outcome preservation, but mandatory breadth and independent acceptance are incomplete. Spec is **PARTIAL**, not PASS. The PR must remain draft and TASK-178 In Progress until every original AC is verified. A review from the PR author's own account is not a distinct independent human review.

## Verification

Local environment has no `cargo`, `rustfmt` or `rustc`. Tests and formatting are **not yet executed here**; GitHub CI status must be checked on the implementation PR head. No external sql-tdg consumer CI is claimed.

## Expanded implementation and executable checks (2026-11-09)

The first seven-case matrix remains immutable as a starter freeze. The additive [full named-variant inventory](task-178-full-variant-inventory.json) explicitly accounts for **163 original TASK-66 SQL variants x 13 dialects = 2,119 cells** across predicate, JOIN, set, window, ordering, schema, DML and DDL families, plus the five historical grouping/LATERAL/STRUCT drift originals. Its pinned observed classifications are 2,014 same-public-document, 23 canonical-drift, 55 Unsupported and 27 parser-rejected. Their source identities, original SQL examples, prior native proof ledger and prior ten-case typed parity evidence are linked individually. **All 2,119 still require semantic/full-domain review**; equal public documents do not assert executed equivalence.

[Full-inventory public tests](../tests/task_178_full_variant_inventory.rs) enforce all record IDs, observed per-dialect statuses, complete public JSON equality where observed, explicit unsupported/parser boundaries and the 23 individually owned drifts. The independent [meaning-preserving synonym fixtures](task-178-syntax-pairs.json) and [native tests](../tests/task_178_sql_synonyms.rs) exercise INNER/JOIN, UNION/UNION DISTINCT (SQL NULL and bag membership), and <>/!= against complete DuckDB bags and all 13 public dialects, while keeping TOP/FETCH/MINUS, window ordering and DML/DDL effect differences as named unapproved or deliberately non-equivalent controls. Three true synonym pairs are **not** a complete replacement for 163 variant-specific proofs.

[Source-adapter typed parity](../tests/task_178_adapter_typed_parity.rs) executes actual direct DDL, compiled dbt source and ODCS schema sources for the integer >= 10 filtering case, independently asserting its inclusive domain, native NULL/duplicate and TRUE/FALSE/UNKNOWN evidence, and typed source/terminal read proof. An unattested dbt producer remains **Unattested**, not native execution. This is **one** independent cross-adapter proof family, not blanket SQL/dbt/ODCS signoff.

[TASK-178 grouping drift regression](../tests/task_178_grouping_ast_equivalence.rs) deliberately tests whether three original ROLLUP/CUBE/GROUPING SQL spellings produce identical complete public documents across parsers; the older TASK-177 frozen drift observation is not silently rewritten or treated as proof. [TOP/FETCH/MINUS syntax probes](task-178-syntax-dialect-boundaries.json) record the pending meaning-preserving versus explicitly non-equivalent distinctions and independent DuckDB EXCEPT/LIMIT NULL/duplicate reference goldens. Their per-dialect masks must be measured, reviewed and frozen; a parser-only pass is insufficient.

**Verified latest fully green intermediate CI:** [run 42709550658](https://github.com/phdah/sql-semantic-protocol/actions/runs/42709550658), 9/9 required jobs at `f930039d47511dcba47f934854f3477abea9b282` (before later red/green drift and syntax probes and new evidence manifest). This is not final PR-head CI, vendor execution or pinned sql-tdg consumer certification.

**Remaining TASK-178 signoff blockers:** Original AC #1 requires full per-cell typed source-instance and complete expected terminal/control goldens, not just 2,119 observed classifications. AC #2 requires correct strongest sound typed canonical equivalents across each finite mandatory family and adjudication of all 23 drifts; merely accepting Unsupported cannot close a mandatory core cell. AC #3 requires independent full positive/rejected/impossible evidence, including mutation state and open source-identity constraints, beyond the finite starter oracles. AC #4 requires representable adapter pairs and individually reviewed conditional parser/native/session laws. AC #5 requires fully green **final-head** checks, pinned sql-tdg and independent reviewer acceptance on every owned cell. No AC is checked, TASK-178 remains In Progress, and TASK-88/TASK-91 release signoffs remain blocked.
