//! Independent TASK-191 second-pass review of all source-bound dialect exceptions.
//! No parser success or Unsupported statement is promoted to a native output proof.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, ProtocolStatement};

const REVIEW: &str = include_str!("../docs/task-191-reviewed-dispositions-v2.json");
const FREEZE: &str = include_str!("../docs/task-191-acceptance-freeze.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const SNAPSHOT: &str = include_str!("../docs/task-177-parser-snapshot.json");
const PARSER: &str = include_str!("../docs/task-177-parser-boundaries.json");
const ANALYZER: &str = include_str!("../docs/task-177-analysis-boundaries.json");
const NATIVE: &str = include_str!("../docs/task-177-remaining-original-cases.json");
const PHYSICAL: &str = include_str!("../docs/task-177-physical-proof-ledger.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");

fn read(s: &str) -> Value {
    serde_json::from_str(s).expect("committed independent source evidence")
}

fn cases<'a>(v: &'a Value, field: &str) -> &'a [Value] {
    v[field].as_array().expect("complete versioned ledger")
}

fn text<'a>(v: &'a Value, field: &str) -> &'a str {
    v[field].as_str().expect("required source-backed claim")
}

fn originals(v: &Value) -> BTreeMap<&str, &Value> {
    cases(v, "cases")
        .iter()
        .map(|case| (text(case, "id"), case))
        .collect()
}

#[test]
fn reviewed_parser_analyzer_and_native_cells_have_independent_owners_and_original_sql() {
    let review = read(REVIEW);
    let freeze = read(FREEZE);
    let samples = read(SAMPLES);
    let snapshot = read(SNAPSHOT);
    let parser = read(PARSER);
    let analyzer = read(ANALYZER);
    let native = read(NATIVE);
    let physical = read(PHYSICAL);
    let manifest = read(MANIFEST);
    let source = originals(&samples);

    assert_eq!(review["schema_version"], 2);
    assert_eq!(review["task_id"], "TASK-191");
    assert_eq!(review["freeze_v1"], "docs/task-191-acceptance-freeze.json");
    assert_eq!(review["all_mandatory_features_implemented"], false);
    assert_eq!(review["independent_native_external_vendor_attested"], false);
    assert_eq!(review["task_88_parent_release_approved"], false);
    assert_eq!(source.len(), 251);
    let dialects = cases(&samples, "dialects");
    assert_eq!(dialects.len(), 13);

    let parser_rows = cases(&review, "parser_cells");
    let unsupported_rows = cases(&review, "unsupported_cells");
    let native_rows = cases(&review, "native_original_cases");
    assert_eq!(parser_rows.len(), 55);
    assert_eq!(unsupported_rows.len(), 116);
    assert_eq!(native_rows.len(), 23);

    let frozen_parser: BTreeSet<_> = cases(&freeze, "parser_rejected_cells")
        .iter()
        .map(|r| text(r, "cell_id"))
        .collect();
    let frozen_unsupported: BTreeSet<_> = cases(&freeze, "analyzer_unsupported_cells")
        .iter()
        .map(|r| text(r, "cell_id"))
        .collect();
    let frozen_native: BTreeSet<_> = cases(&freeze, "native_unproven_original_cases")
        .iter()
        .map(|r| text(r, "original_id"))
        .collect();
    let independently_observed_parser: BTreeSet<_> = cases(&parser, "cells")
        .iter()
        .map(|r| text(r, "id"))
        .collect();
    let independently_observed_analyzer: BTreeSet<_> = cases(&analyzer, "cells")
        .iter()
        .map(|r| text(r, "id"))
        .collect();
    let independently_unproved_native: BTreeSet<_> = cases(&physical, "cases")
        .iter()
        .filter(|r| r["native_duckdb_original_sql"] == "not_proven")
        .map(|r| text(r, "id"))
        .collect();

    assert_eq!(frozen_parser, independently_observed_parser);
    assert_eq!(frozen_unsupported, independently_observed_analyzer);
    assert_eq!(frozen_native, independently_unproved_native);
    assert!(frozen_parser.is_disjoint(&frozen_unsupported));

    let mut observed = BTreeSet::new();
    for (kind, rows, expected_set, initial_rows, matrix, expected_mask) in [
        (
            "parser",
            parser_rows,
            &frozen_parser,
            cases(&freeze, "parser_rejected_cells"),
            &snapshot["matrix"],
            'R',
        ),
        (
            "unsupported",
            unsupported_rows,
            &frozen_unsupported,
            cases(&freeze, "analyzer_unsupported_cells"),
            &snapshot["canonical_analysis_matrix"],
            'U',
        ),
    ] {
        let mut ids = BTreeSet::new();
        for (index, row) in rows.iter().enumerate() {
            let id = text(row, "cell_id");
            let original_id = text(row, "original_id");
            let original = source.get(original_id).expect("immutable original SQL");
            let dialect = text(row, "dialect");
            assert_eq!(
                row["original_sql"], original["sql"],
                "{id}: source text drift"
            );
            assert_eq!(row["feature_id"], original["feature_id"]);
            assert_eq!(row["variant"], original["variant"]);
            assert_eq!(row["semantic_owners"], original["semantic_owner"]);
            assert_eq!(row["cell_id"], initial_rows[index]["cell_id"]);
            let feature = cases(&manifest, "features")
                .iter()
                .find(|feature| feature["id"] == row["feature_id"])
                .expect("advertised original feature");
            assert_eq!(feature["scope"], "release_blocking");
            assert_eq!(row["feature_scope"], "release_blocking");
            assert_eq!(row["external_vendor_native_execution"], "not_attested");
            assert!(text(row, "named_limitation").len() > 20);
            let d = dialects
                .iter()
                .position(|entry| entry == dialect)
                .expect("known public dialect");
            let mask = matrix[original_id]
                .as_str()
                .expect("original observed mask");
            assert_eq!(mask.chars().nth(d), Some(expected_mask));
            assert_eq!(
                row["original_sql_source"],
                "docs/task-177-parser-samples.json"
            );
            assert!(ids.insert(id), "{id}: duplicate reviewed disposition");
            assert!(observed.insert(id), "{id}: cross-axis collision");
            match kind {
                "parser" => {
                    assert_eq!(
                        row["original_parser_proof"],
                        "Impossible_for_original_sql_with_sqlparser_0_58"
                    );
                    assert_eq!(row["value_domain"], "Unknown_no_statement_value_emitted");
                    assert_eq!(
                        row["release_decision"],
                        "blocked_feature_remains_release_required"
                    );
                }
                "unsupported" => {
                    assert_eq!(row["statement_semantics"], "Unsupported");
                    assert_eq!(
                        row["typed_output_policy"],
                        "Unknown_no_strong_domain_proven"
                    );
                    assert_eq!(row["approved_as_supported_or_deferred"], false);
                    assert_eq!(
                        row["release_decision"],
                        "blocked_unimplemented_mandatory_semantics"
                    );
                }
                _ => unreachable!("only two original public classes"),
            }
        }
        assert_eq!(&ids, expected_set);
    }

    let mut reviewed_native = BTreeSet::new();
    for (index, row) in native_rows.iter().enumerate() {
        let id = text(row, "original_id");
        let original = source.get(id).expect("immutable original SQL");
        assert_eq!(
            row["original_id"],
            cases(&freeze, "native_unproven_original_cases")[index]["original_id"]
        );
        assert_eq!(row["original_sql"], original["sql"]);
        assert_eq!(row["semantic_owners"], original["semantic_owner"]);
        let frozen_case = cases(&native, "cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("historical original execution limitation");
        assert_eq!(row["proof_obligation"], frozen_case["proof_obligation"]);
        assert_eq!(row["original_fixed_terminal_certified"], false);
        assert_eq!(row["external_vendor_native_execution"], "not_certified");
        assert_eq!(
            row["release_decision"],
            "reviewed_original_execution_boundary_release_blocking"
        );
        assert!(reviewed_native.insert(id));
    }
    assert_eq!(reviewed_native, frozen_native);
}

#[test]
fn every_original_unsupported_diagnostic_is_source_bound_and_precisely_regressed() {
    let review = read(REVIEW);
    let samples = read(SAMPLES);
    let source = originals(&samples);
    let mut observed = BTreeSet::new();
    for row in cases(&review, "unsupported_cells") {
        let id = text(row, "cell_id");
        let original_id = text(row, "original_id");
        let dialect = text(row, "dialect");
        let original = source.get(original_id).expect("immutable original");
        let parser = dialect_from_name(dialect).expect("registered dialect");
        let protocol = analyze_sql(text(original, "sql"), dialect, parser.as_ref())
            .unwrap_or_else(|error| panic!("{id}: expected public Unsupported, got {error}"));
        let expected_category = text(row, "expected_unsupported_category");
        let expected_code = text(row, "expected_diagnostic_code");
        let expected_message = text(row, "expected_diagnostic_message");
        let expected_area = text(row, "diagnostic_area");
        let expected_severity = text(row, "diagnostic_severity");
        let unsupported: Vec<_> = protocol
            .statements()
            .iter()
            .filter_map(|statement| match statement {
                ProtocolStatement::Unsupported(value) => Some(value),
                _ => None,
            })
            .collect();
        assert!(
            !unsupported.is_empty(),
            "{id}: missing explicit Unsupported"
        );
        assert!(
            unsupported.iter().any(|statement| {
                statement.category() == expected_category
                    && statement.diagnostics().iter().any(|diagnostic| {
                        diagnostic.code() == expected_code
                            && diagnostic.message() == expected_message
                            && format!("{:?}", diagnostic.area()) == expected_area
                            && format!("{:?}", diagnostic.severity()) == expected_severity
                            && !diagnostic.message().is_empty()
                    })
            }),
            "{id}: changed unsupported category/code; review semantic owner before updating evidence"
        );
        assert!(
            observed.insert(id),
            "{id}: duplicated independent diagnostic review"
        );
    }
    assert_eq!(observed.len(), 116);
}
