---
id: TASK-104
title: 'Certify pattern, collation and null-safe expression laws'
status: In Progress
assignee: []
created_date: '2026-10-13'
updated_date: '2026-10-15 18:17'
labels: []
milestone: m-3
dependencies:
  - TASK-103
references:
  - TASK-70
  - TASK-88
  - TASK-66
priority: high
type: feature
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Complete the scoped text/pattern and dialect-specific comparison audit. Exact claims require explicit operator, datatype, collation, padding and dialect/session evidence; unsupported regex or pattern modes remain intentionally residual.

**TASK-70 child, step 4/8.** Implement this task in a fresh PR targeting `main` once the previous dependency has merged. The TASK-70 parent owns the [dependency-ordered queue](task-70%20-%20solve-expression-and-predicate-variants.md); this child MUST NOT depend on the parent (no dependency cycle). A green, bounded increment is mergeable but does not complete this child unless every criterion below is independently verified.

**Release contract:** Produce canonical, source-independent, parser-agnostic typed outcomes with strongest provable value domains and deterministic proof strength (exact, impossible, residual) for SQL TRUE and NOT TRUE. Preserve explicit residuals and never move missing semantics to sql-tdg. Protocol 3.0.0 release remains gated by TASK-91.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria

<!-- AC:BEGIN -->
- [ ] #1 Implement exact LIKE/NOT LIKE, supported ESCAPE variants and provably safe nonprefix patterns where typed binary/no-padding and wildcard/escape laws suffice; preserve SQL UNKNOWN for NULL operands. Audit ILIKE and regex-family spellings and flags for each parser-capable dialect.
- [ ] #2 Represent collation, case folding, CHAR padding, Unicode, comparison ordering and null-safe equality assumptions as explicit canonical evidence/preconditions, not inferred vendor defaults; handle MySQL `<=>` and relevant AST variants only under proven laws.
- [ ] #3 Prove TRUE/NOT TRUE, impossible, residual, precise value-domain constraints and output comparison settings for safe text patterns, cross-text-column equality where attested and interactions with mixed Boolean predicates.
- [ ] #4 Enumerate and explicitly classify every inventoried dialect-sensitive text/operator variant as exact under certified conditions or fail-closed residual with a reason. Never claim regex engine or vendor collation parity based solely on a successful parser.
- [ ] #5 Add shared cross-dialect canonical equivalence, syntax-boundary cases, dbt evidence parity, DuckDB pattern/collation comparison oracles and negative/NULL/ESCAPE counterexamples. Update `docs/coverage-manifest.json`, contract, schema and tests.
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Research (done): LIKE only safe trailing-% ascii prefix exact under binary_collation+no_char_padding; ILIKE/SIMILAR/RLIKE/PG-regex/BinaryOp-regex/COLLATE/LIKE ANY/unsafe ESCAPE all fall to BooleanExpression residual; Spaceship (<=>) unmapped residual; cross-column only integers; CHAR fixed excluded from string evidence. Seams (repo-convention, integration public API only): analyze_sql / analyze_configured_inputs_with_catalog + declare_comparison_assumptions -> BooleanWitness directions/condition; to_json/to_bundle_json wire shape vs schema/protocol.schema.json; DuckDB oracle; dbt parity via analyze_dbt_artifacts. 2. TDD slice A (parser): exact LIKE without wildcard as StringEquality (Eq/Neq incl NOT LIKE, NULL->UNKNOWN) with inert-ESCAPE law; test first in tests/task_104_pattern_collation_nullsafe.rs. 3. TDD slice B (null-safe): map BinaryOperator::Spaceship to IsNotDistinctFrom; integer + attested-string exact, else residual; unparenthesized parser-boundary stays residual. 4. TDD slice C (cross-text-column): new BooleanRowConstraint::StringColumnComparison (Eq/Neq/IsDistinct/IsNotDistinct, both sides catalog VARCHAR + attestations); wire normalize/negate/not_true/enumeration/eval/emission/schema; mixed AND/OR/NOT with LIKE/equality exact in both directions. 5. TDD slice D (audit matrix): enumerate LIKE/NOT LIKE/ESCAPE safe+unsafe/ILIKE/SIMILAR/RLIKE-REGEXP/PG ~,~,!,~~ variants/LIKE ANY/COLLATE/CHAR-vs-VARCHAR/Unicode/_-wildcard/%-interior/order comparisons (<,<=,>,>= on text residual)/<=> per-dialect parse matrix across common::DIALECTS with exact-vs-residual classification, never engine-parity claims. 6. TDD slice E (oracles/parity): DuckDB LIKE/collation comparison oracle (NULL/NOT LIKE/ESCAPE/cross-text counterexamples), dbt compiled-SQL parity, cross-dialect canonical equivalence, negative/NULL/ESCAPE counterexamples, output-domain checks (qualifying Empty on impossible, unbounded elsewhere) + comparison-settings emission. 7. Docs/contract: update docs/semantics.md (pattern/collation/nullsafe laws), docs/coverage-manifest.json (proven variants + family_evidence: extend string_binary_equality_and_like_prefix scope/evidence, promote dialect_specific_nullsafe_operators to bounded with residual remainder, keep regex/nonbinary residual with new audit evidence), docs/code-map.md if ownership shifts, schema/protocol.schema.json (string_column_comparison) + docs/protocol.md if shape changes, tests/task_70_predicates.rs inventory asserts. 8. Verify: cargo fmt, clippy --all-targets --all-features -- -D warnings regularly + per-slice single-file tests, full cargo test once at end, cargo doc if public docs change.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Two-axis review completed. Standards: fixed DRY duplication (single Output::empty_on_impossible owner in protocol/relations.rs), replaced unreachable! with an exhaustive match in the new eval arm, kept double-eval guard style consistent with the pre-existing prefix code. Spec: extended the 52-row variant matrix across all 14 parser-capable dialects (728 analyses, zero parser boundaries; every spelling parses everywhere and classification holds), added the unparenthesized null-safe text parser-boundary residual, added dbt residual-pattern parity alongside the exact spaceship parity. Rebutted with evidence: output-emptying on attestation-time Impossible is required by AC#3 plus the project-wide outcome-first DoD (leaving Unknown when Empty is proven would be a wrong domain); <=> parses in all 14 sqlparser dialects (token-level, verified in parser source) so the shared-shape assertion records parser-boundary fact, not engine parity; space/punctuation patterns stay residual under the documented portable ASCII-alphanumeric law while regex/collation modes remain intentionally residual. Verified: cargo fmt, clippy -D warnings clean, cargo doc clean, 12/12 task_104 green, full cargo test 993 passed 0 failed across 72 binaries.
<!-- SECTION:NOTES:END -->

## Implementation and verification

- **Parent:** [TASK-70](task-70%20-%20solve-expression-and-predicate-variants.md), still **In Progress** until TASK-108 verifies all six original acceptance criteria.
- **Delivery:** one focused PR to `main`; finish all the child AC before marking it Done and verify complete relevant CI after implementing (not after every commit). Update canonical API/schema/docs, adapter parity and coverage evidence as applicable.
- **Scope boundary:** Local comparison/operator laws only. Full 13-dialect matrix remains TASK-88; external unavailable vendor-engine execution remains unverified rather than guessed.
- **Next:** After completion, pick **TASK-105**.
