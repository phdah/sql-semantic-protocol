# TASK-126 independent acceptance sign-off

Reviewed: 2026-10-20. Scope: PR #136, TASK-126 only. TASK-74 remains open; this sign-off does not approve the 3.0.0 release.

## Evidence and immutable test contract

- Frozen matrix: `docs/task-126-acceptance-freeze.json`, v1, fixture families `74-C126-P` and `74-C126-N`.
- Exact cases: `74-C126-P1` (LAG/LEAD, NULLs), `74-C126-P2` (FIRST/LAST/COUNT(*) and ROWS PRECEDING), `74-C126-N1` (following frame, duplicates, empty result). Each has a complete physical source bag, full terminal multiset, row identities and exact finite nullable domains with inclusive integer bounds.
- Conditional `Residual` cases: `74-C126-C1` peer ambiguity, `C2` implicit NULL order, `C3` RANGE frame, `C4` implicit/default frame, `C5` dynamic LAG offset. Owner: TASK-126; no vendor/session execution claim. Advanced frames and vendor laws remain TASK-128.
- Parser matrix: eight cases multiplied by all 13 exposed dialects = **104 explicit parser-status cells**, all classified `parsed` and checked against sqlparser. The three mandatory cases additionally require `Exact` physical proofs in every dialect; the five conditional cases require `Residual`.
- Adapter matrix: eight cases multiplied by direct SQL, compiled dbt and ODCS = **24 required adapter cells**. The three exact cases require identical complete source identities, terminal bags and output domains on all adapters; conditional cases require `Residual`. The adapter test uses PostgreSQL for the same canonical SQL with separately supplied schema evidence.
- Independent executable oracle: `tests/task_126_window_bags.rs` creates a real in-memory DuckDB source, checks every source row including NULL and duplicates, executes the frozen SQL and compares entire terminal bags. It separately tests absent source as `Residual` and invalid signed-integer source as `Impossible`. The domain checks assert every observed finite value, SQL NULL, min/max and bound inclusivity.
- `tests/task_126_window_adapters.rs` checks compiled manifest + ODCS versus direct SQL; `frozen_acceptance_matrix_has_no_implicit_parser_or_adapter_exclusions` rejects unrecorded cells.

## Review note on the acceptance freeze

The original v1 fixture annotations named `id INTEGER NOT NULL` even though the direct SQL/dbt/ODCS catalog paths supply an integer datatype without enforced nullability. The annotation has been corrected to `INTEGER` to avoid claiming unprovided evidence; **the actual source and terminal bags, oracle values, required proof classes and numeric bounds have not changed**. Explicit dialect parser statuses have also been frozen to prevent conditional parser failures silently passing CI.

## Original AC crosswalk

| Child AC | Verified evidence |
| --- | --- |
| #1 Freeze bounded typed cases before implementing | Commit history freezes v1 before semantic edits; matrix and contract test cover eight cases, SQL, parser, schema, row identity, positive/NOT TRUE, domains, dialects, adapters, and named conditional disposition |
| #2 Canonical/physical exact core | `src/physical_realization/window_bags.rs` evaluates parser-independent window AST over a complete schema-validated source, preserving NULL, duplicates, identities, finite bounds, and `Exact`/`Impossible`/`Residual` |
| #3 Independent DuckDB source and terminal | All P1/P2/N1 complete source/terminal bags and bounds, SQL NULL, duplicate values, empty frame and contradictory source are asserted |
| #4 All dialects and equivalent adapters | 104 explicit parser cells and 24 adapter proof cells with fail-closed conditional boundaries |
| #5 Final gates and original parent mapping | Verified against CI run [3316](https://github.com/phdah/sql-semantic-protocol/actions/runs/39411757453) at code head `d7f67b2abda9dde1990bbaf714374352c03705a7`. All jobs succeeded: formatting, strict lint, full/no-default-features tests, rustdoc, dependency audit, dbt Core E2E. Parent AC #1, #2, #4, #5, #6 are mapped to this child only; parent AC #3 and remaining variants are **not** signed off |

## Disposition

**TASK-126 acceptance criteria independently verified.** This audit and task/manifest metadata are nonsemantic additions; CI must remain green on the resulting PR head before merge. Leave the PR unmerged for maintainer review. Keep TASK-74 open, TASK-189 gated on TASK-126 merging to `main`, and 3.0.0 release unapproved.
