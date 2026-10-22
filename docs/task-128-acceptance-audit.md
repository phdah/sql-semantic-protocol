# TASK-128 bounded advanced-window acceptance audit (PR #139)

**Review date:** 2026-10-22. **Scope:** TASK-128, the final dependency-ordered child of TASK-74. Parent TASK-74 and release gate TASK-91 are independent and are not approved through this audit. The signed-off earlier children TASK-125, TASK-126, TASK-189, and TASK-127 are already merged on `main`.

## Frozen physical cases and evidence

The task-owned [v4 matrix](task-128-acceptance-freeze.json) retains fixed fixture-family IDs `74-C128-P`/`74-C128-N`, seven independent physical rows with duplicate peer values and NULL, source identities, typed inclusive terminal sets and full expected multisets. No conditional case is allowed to substitute for a mandatory Exact case.

| Case | Classification | Executed evidence |
| --- | --- | --- |
| P1 | Exact NTILE(3) balanced 5- and 2-row partitions | `tests/task_128_advanced_windows.rs`: source/terminal bag, row IDs, typed NTILE domain, independent DuckDB |
| P2 | Exact cumulative RANGE with duplicate ORDER BY peers and explicit NULLS LAST | Same full bag, source and typed finite-domain DuckDB oracle |
| P3 | Exact moving GROUPS 1 PRECEDING peer-frame membership | Same, plus real compiled dbt Core source and model warehouse execution |
| P4 | Exact NTILE QUALIFY = 1: 3 TRUE, 4 NOT TRUE physical rows | Same public SQL API + DuckDB and real dbt Core native execution |
| P5 | Exact cumulative RANGE QUALIFY > 20: 3 TRUE, 4 NOT TRUE rows | Same, preserving duplicates and NULL-aware membership |
| N1 | Conditional Residual for invalid zero NTILE count | Physical evaluator refuses an unsupported bucket law; not marked an Impossible source contradiction |
| N2 | Conditional Residual for NTILE peers with no total ORDER BY | Physical evaluator never chooses one peer's row identity arbitrarily |
| N3 | Conditional Residual for implicit NULL ordering in GROUPS | Native DuckDB complete bag is executed separately; no portable default claim |
| N4 | Conditional Residual for bounded numeric RANGE offsets | Native DuckDB full bag is independently checked; no inferred portable arithmetic-distance/session law |
| N5 | Conditional Residual for multi-key GROUPS with implicit NULL placement | Native DuckDB complete bag is independently checked; canonical proof stays Residual |
| N6 | Conditional Residual for CAST/COLLATE-dependent NTILE ordering | No typed engine-independent collation comparison law is inferred |

`tests/task_128_advanced_windows.rs` also independently constructs an out-of-range physical signed-integer source and requires `Impossible`, while an absent authoritative source stays `Residual`. All supported exact cases conserve complete source identities, terminal identities and NOT TRUE membership; no source is silently filtered or assigned a guessed peer order.

## Dialects, adapters, vendor boundaries

- The public parser/canonical equivalence regression executes every recorded frozen query under **all 13 dialects**. The frozen expected parser status is asserted, not accepted from a successful parse alone; each parsed mandatory case is required to reproduce its whole exact canonical physical bag. Parser equivalence is not a claim of native execution in 13 database engines.
- `tests/task_128_adapter_parity.rs` compares direct SQL, normalized compiled dbt manifests with source schemas, and ODCS source contracts through the same public physical proof API. ODCS is *schema evidence*, never a SQL runtime. No cross-adapter consumer reparse is used to fabricate a proof.
- `tests/task_128_dbt_core_e2e.rs` is executed under `make dbt-e2e`: real seed/model, compiled `manifest.json`/`catalog.json`, independent native complete source and terminal bags, exact physical identities and typed domains for NTILE and GROUPS. It also checks the actual pre-existing `ranked_orders.sql` fixture's entire joined producer bag and terminal values (ROW_NUMBER, ROWS SUM and QUALIFY).
- `tests/task_128_advanced_windows.rs` runs native DuckDB conditional full-bag oracles for implicit-NULL GROUPS, bounded numeric RANGE and composite GROUPS, while affirming the canonical result stays Residual. Other vendor engines and collation sessions are **not** certified or claimed as executable without independent native oracles. These are explicitly reviewed conditional residual boundaries, not omitted mandatory exact cases.
- The existing `tests/dbt_core_e2e.rs` golden-snapshot gate remains unchanged in meaning: TASK-128 models are excluded from its legacy comparison and tested independently instead. The native dbt model inventory includes both new models.

## Original parent TASK-74 AC #1 through #6 crosswalk

1. **Ranking, navigation and aggregates:** TASK-125 certifies ROW_NUMBER/RANK/DENSE_RANK; TASK-126 certifies LAG/LEAD, FIRST/LAST and bounded ROWS; TASK-189 certifies COUNT/SUM/AVG/MIN/MAX with typed windows. TASK-128 supplies NTILE and advanced frames.
2. **Ordering, peers and frames:** TASK-125/126 establish deterministic partition/ordered ties and NULL/ROWS. TASK-128 adds RANGE peer-inclusive and GROUPS peer membership, explicit NULLS LAST for Exact, and typed Residual for implicit NULL defaults, CAST/COLLATE and value-distance assumptions.
3. **TRUE and NOT TRUE QUALIFY:** TASK-127 certifies multiple comparisons and complete joined/grouped upstream contributors; TASK-128's P4/P5 add advanced window filters with exact physical survivors and rejects.
4. **No invented order:** TASK-125/127/128 fail closed on missing total order where row identity depends on ties. GROUPS/RANGE peer membership remains exact only when ordering laws are explicit and type-attested.
5. **Native and project oracles:** Each previous child has independent DuckDB oracles. TASK-128 adds NTILE, GROUPS, conditional native frame matrices and executed, pre-existing `ranked_orders.sql` through dbt Core E2E.
6. **Typed, cross-dialect and adapter conformance:** The child suite certifies outcomes, multiplicity, NULL, Impossible and Residual boundaries, 13 parser paths, SQL/dbt/ODCS parity and complete end-to-end fixture execution. No public serialized schema was changed by the TASK-128 implementation; the canonical physical API was extended internally.

## Closure conditions

All five mandatory Exact cases are independently supported by full source and terminal physical/output bags, five of the eleven frozen IDs include explicit TRUE/NOT TRUE or impossible contradictions as required by their classification, 143 dialect/parser and 33 adapter cells have executable public parity tests, and all remaining conditional runtime gaps are conservative typed Residuals with TASK-128 ownership. The nine-job [semantic-head CI #3498](https://github.com/phdah/sql-semantic-protocol/actions/runs/39751701868) succeeded. The above is the independent case-by-case audit of child evidence. Mark TASK-128 Done **only after final-head** formatting, strict Clippy, default/no-default feature tests, docs, dependency checks and `make dbt-e2e` all pass, conditional vendor/session dispositions are explicitly owned, and no frozen mandatory case is unverified.

Do not mark TASK-74 Done until TASK-128 merges into main and a separate independent review verifies the parent original AC #1–#6 against all **five merged child** evidence sets. Do not approve release gate TASK-91 as a side effect.
