# TASK-107 conditional semantic review (2026-10-17)

This is the bounded technical disposition of the fourteen frozen conditional
cells in `docs/task-107-acceptance-freeze.json`. **A parse or equal SQL/dbt
emission never attests vendor runtime semantics.** All dispositions preserve
the existing TASK-88 global cross-dialect gate and TASK-91 release gate.
`reviewed` means the *current protocol boundary* was examined, not that
unsupported behavior was implemented or that a proprietary engine was tested.

| Cell | Reviewed protocol boundary | Evidence and surviving limitation |
| --- | --- | --- |
| E107-V021 ILIKE | Residual without case-folding/Unicode laws. Never substitute binary LIKE. | `tests/task_104_pattern_collation_nullsafe.rs::inventoried_variants_have_explicit_exact_or_residual_classification` |
| E107-V023 SIMILAR TO | Residual across engines' regex languages and flags. | `tests/task_104_pattern_collation_nullsafe.rs::inventoried_variants_have_explicit_exact_or_residual_classification` |
| E107-V024 regex | Residual, including opaque `REGEXP_LIKE`; parser acceptance does not grant a regex runtime law. | `tests/task_104_pattern_collation_nullsafe.rs::inventoried_variants_have_explicit_exact_or_residual_classification`, `tests/task_105_temporal_json_vendor.rs::opaque_function_domains_and_predicate_residuals_name_the_function` |
| E107-V026 COLLATE | Explicit locale comparison is not certified binary collation. Only separately attested binary/no-padding equality is exact. | `tests/task_104_pattern_collation_nullsafe.rs::inventoried_variants_have_explicit_exact_or_residual_classification` |
| E107-V032 scalar UDF | Unknown output and residual predicate; never guess UDF purity or NULL law. | `tests/task_105_temporal_json_vendor.rs::opaque_function_domains_and_predicate_residuals_name_the_function` |
| E107-V034 temporal family | Gregorian DATE literal identity and literal YEAR/MONTH/DAY extraction remain mandatory exact. TIME/TIMESTAMP/INTERVAL conversion and arithmetic outside proved preconditions remain unknown. | `tests/task_105_temporal_json_vendor.rs::date_literal_calendar_fields_are_folded_to_exact_integer_domains`, `tests/task_105_temporal_json_vendor.rs::inventoried_variants_are_certified_or_fail_closed_in_every_exposed_dialect` |
| E107-V035 timezone | Offset/zone inference requires explicit semantics, source zone, and clock; unsupported session/DST conversion stays unknown. | `tests/task_105_temporal_json_vendor.rs::inventoried_variants_are_certified_or_fail_closed_in_every_exposed_dialect` |
| E107-V036 coercion | Validated literal DATE conversion is exact; arbitrary string, precision, implicit and session-dependent casts remain unknown. | `tests/task_105_temporal_json_vendor.rs::explicit_date_literal_identity_cast_has_an_exact_calendar_domain`, `tests/task_105_temporal_json_vendor.rs::inventoried_variants_are_certified_or_fail_closed_in_every_exposed_dialect` |
| E107-V037 DST / interval | No portable fold/gap, time-zone or interval arithmetic rule is certified; six syntax rejections remain parser-only evidence. | `tests/task_105_temporal_json_vendor.rs::unsupported_temporal_and_nested_ast_shapes_expose_their_family`, `tests/task_107_dialect_classifications.rs::all_frozen_dialect_cells_are_actually_parsed_and_equivalence_checked` |
| E107-V038 binary/locale | Binary equality only exact when both binary collation and no CHAR-padding assumptions are declared; locale comparisons without attestation remain residual. | `tests/task_104_pattern_collation_nullsafe.rs::cross_text_column_equality_is_exact_only_when_attested` |
| E107-V039 padding | Fixed-width CHAR comparison cannot inherit VARCHAR byte-equality, even with binary/no-padding assumptions. | `tests/task_104_pattern_collation_nullsafe.rs::fixed_width_char_columns_never_claim_binary_laws` |
| E107-V040 NaN | Floating NaN equality and ordering require vendor/runtime law; do not infer IEEE engine behavior from integer comparison. | `docs/semantics.md` (explicit unknown comparison environment), `tests/task_107_dialect_classifications.rs::all_frozen_dialect_cells_are_actually_parsed_and_equivalence_checked` (syntax only) |
| E107-V041 signed zero | Floating signed-zero equality, bit identity and ordering have no attested dialect law; no integer-law substitution. | `docs/semantics.md` (comparison assumptions), `tests/task_107_dialect_classifications.rs::all_frozen_dialect_cells_are_actually_parsed_and_equivalence_checked` (syntax only) |
| E107-V042 NULL sorting | `NULLS FIRST/LAST` parse classifications do not establish vendor default ordering or physical realization. This is not a Boolean predicate. | `tests/task_107_dialect_classifications.rs::all_frozen_dialect_cells_are_actually_parsed_and_equivalence_checked` (syntax only) |

## Signoff interpretation

- **Current TASK-107 disposition:** accept the documented typed residual
  or guarded-exact boundary, not new engine-semantic coverage. Reviewed conditional
  cells remain attributed to TASK-88 for **new** runtime/dialect proof, and the
  coverage manifest must not be upgraded to all-dialect constructive proof.
- **Five mandatory projected cells:** `E107-V028/029/030/031/033` require a
  separate public-API and complete physical DuckDB oracle:
  `tests/task_107_projection_proofs.rs` and
  `tests/task_107_frozen_execution_oracles.rs`.
  Source predicates narrow finite domains before claiming an exact set;
  notably unguarded narrowing `TRY_CAST` is **Unknown**, not Exact.
- **Release:** this review does not authorize protocol 3.0.0, certify
  proprietary engines, mark TASK-88/TASK-89 complete, or waive the
  final-head CI requirement.
