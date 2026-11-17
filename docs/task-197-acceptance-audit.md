# TASK-197 cross-adapter evidence audit (in progress)

## Frozen inventory

- Owner: TASK-197, parent TASK-88. Positive/negative fixture IDs: `88-C197-P`, `88-C197-N`.
- Source of truth: `docs/task-178-full-variant-inventory.json` and the five completed task-owned freezes (TASK-192..196).
- Ledger: `docs/task-197-acceptance-freeze.json` preserves all **163 original SQL variants x 13 dialects = 2,119 cells**.
- Historical parser observations: C=2,014, D=23, R=27, U=55. These are **not** execution proof or SQL-vendor equivalence.
- Every cell retains an explicit conservative classification: C/D are conditional pending TASK-197 review and TASK-191/TASK-179 adjudication where needed; R/U are unrepresentable for that parser path. No parser success implies native vendor execution.
- Each case records original SQL, family owner, source relations mentioned, direct DDL test, compiled dbt evidence test where relevant, ODCS evidence test where relevant, independent semantic review reference, and the source-fact scope. DML/DDL adapter metadata does **not** represent the original write program or physical execution.

## Test seam

`tests/task_197_cross_adapter_ledger.rs` exercises the versioned *public source-configured analysis* API. It reconciles five original child freezes against the original 163-case inventory, verifies cell statuses and disallows silently approving vendor runtime or missing parser evidence. It also compares typed terminal domains for original cases S001, S043, S066, S106, S191 through direct SQL DDL, compiled dbt model+manifest source metadata, and ODCS metadata on each of 13 supported parser dialects. The sample is 65 parser cells; adapter fact comparisons are metadata equivalence only. The existing TASK-192..196 tests provide broader family-specific native, domain and adapter evidence but are **not** a substitute for the exhaustive case-level TASK-197 signoff.

## Unverified gates (AC #1..#5 remain open)

1. Replace provisional C/D dispositions with individual independently reviewed full canonical typed and physical parity outcomes or explicit justified nonrepresentability. Only equal source evidence can be compared; missing metadata must be separately recorded, not treated as equal.
2. Reconcile all 2,119 cells with independent TASK-192..196 domain, NULL, interval inclusivity, source-occurrence and native terminal-bag goldens. Parser-native gaps remain owned by TASK-191 and vendor/session laws by TASK-179.
3. Complete original SQL, dbt manifest/catalog, ODCS and producer/write-state public API evidence for every source-capable cell. Metadata cannot attest DML or DDL execution.
4. Run exact-head rustfmt, strict Clippy, default/no-default Rust tests and docs, dbt E2E, active protocol JSON schema snapshots and pinned sql-tdg consumer checks.
5. Obtain independent standards and specification review on final tested head and record reviewed source provenance. TASK-180 performs global matrix certification; TASK-88 and TASK-91 retain independent signoff.

**Signoff:** Not approved. No TASK-197 acceptance checkbox or parent/release gate is marked complete.
