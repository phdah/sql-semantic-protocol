//! TASK-196 immutable schema, DML and DDL original-SQL acceptance boundary.
//!
//! Canonical equality (C) is only a parser/analyzer observation. Native state,
//! mutation effects, adapter authority and external execution have separate gates.

use std::collections::BTreeSet;

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const FREEZE: &str = include_str!("../docs/task-196-acceptance-freeze.json");
const ORIGINAL: &str = include_str!("../docs/task-178-full-variant-inventory.json");
const DML: &str = include_str!("../docs/task-177-write-oracles.json");
const DDL: &str = include_str!("../docs/task-177-schema-lifecycle-oracles.json");
const MANIFEST: &str = include_str!("../docs/coverage-manifest.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned task-owned acceptance evidence")
}

fn normalized(mut document: Value) -> Value {
    for input in document["inputs"]
        .as_array_mut()
        .expect("one or more public protocol inputs")
    {
        input["dialect"] = Value::Null;
    }
    document
}

#[test]
fn every_original_variant_and_parser_cell_is_frozen_with_a_named_proof_gate() {
    let frozen = read(FREEZE);
    let originals = read(ORIGINAL);
    let manifest = read(MANIFEST);
    let dml = read(DML);
    let ddl = read(DDL);
    assert_eq!(frozen["schema_version"], 1);
    assert_eq!(frozen["task_id"], "TASK-196");
    assert_eq!(frozen["case_count"], 38);
    assert_eq!(frozen["parser_cells"], 494);
    assert_eq!(frozen["dialects"], originals["dialects"]);
    assert_eq!(
        frozen["family_counts"],
        serde_json::json!({
            "schema": 11, "dml": 18, "ddl": 9
        })
    );
    assert_eq!(
        frozen["observed_parser_cells"],
        serde_json::json!({
            "C": 428, "R": 11, "U": 55
        })
    );
    assert_eq!(frozen["positive_fixture_id"], "88-C196-P");
    assert_eq!(frozen["negative_fixture_id"], "88-C196-N");
    assert_eq!(frozen["independently_reviewed_cells"], 0);
    assert_eq!(frozen["task_complete"], false);
    assert_eq!(frozen["release_approved"], false);

    let expected: BTreeSet<&str> = originals["cases"]
        .as_array()
        .expect("TASK-178 original samples")
        .iter()
        .filter(|case| {
            case["feature_id"].as_str().is_some_and(|family| {
                family.starts_with("schema.")
                    || family.starts_with("dml.")
                    || family.starts_with("ddl.")
            })
        })
        .map(|case| case["id"].as_str().expect("source variant"))
        .collect();
    assert_eq!(expected.len(), 38);

    let mut seen_cases = BTreeSet::new();
    let mut seen_cells = BTreeSet::new();
    let mut native_dml = 0;
    let mut native_ddl = 0;
    let mut named_boundaries = 0;
    for case in frozen["cases"].as_array().expect("task-owned cases") {
        let id = case["id"].as_str().expect("unchanged source id");
        assert!(seen_cases.insert(id), "{id}: duplicate original case");
        let source = originals["cases"]
            .as_array()
            .expect("original source SQL")
            .iter()
            .find(|entry| entry["id"] == id)
            .expect("TASK-178 original cannot be replaced");
        assert_eq!(case["sql"], source["sql"], "{id}: original SQL drift");
        assert_eq!(
            case["feature_id"], source["feature_id"],
            "{id}: family drift"
        );
        assert_eq!(case["variant"], source["variant"], "{id}: variant drift");
        assert_eq!(
            case["original_observed_mask"], source["full_document_classification"],
            "{id}: parser history must not be rewritten"
        );

        let oracle = &case["native_oracle"];
        match oracle["kind"]
            .as_str()
            .expect("explicit oracle classification")
        {
            "original_native_dml_complete_target_bag" => {
                let source = dml["cases"]
                    .as_array()
                    .expect("existing original native DML oracles")
                    .iter()
                    .find(|entry| entry["id"] == id)
                    .expect("native DML oracle must exist");
                assert_eq!(
                    oracle["expected_poststate_bag"],
                    source["expected_terminal_bag"]
                );
                assert_eq!(
                    oracle["expected_returning_bag"],
                    source["expected_returning_bag"]
                );
                native_dml += 1;
            }
            "original_native_schema_lifecycle_source_and_terminal" => {
                let source = ddl["cases"]
                    .as_array()
                    .expect("existing native schema/DDL oracles")
                    .iter()
                    .find(|entry| entry["id"] == id)
                    .expect("schema oracle must exist");
                assert_eq!(oracle["before_bag"], source["expected_source_bag"]);
                assert_eq!(oracle["terminal_bag"], source["expected_terminal_bag"]);
                assert_eq!(oracle["must_reject_after"], source["must_reject_after"]);
                native_ddl += 1;
            }
            "metadata_adapter_only_requires_authoritative_fixture"
            | "unapproved_vendor_or_unsupported_dml_boundary" => {
                assert!(oracle["expected_poststate_bag"].is_null());
                assert_eq!(oracle["review_status"], "unapproved");
                assert!(oracle["boundary_owner"].is_string());
                named_boundaries += 1;
            }
            kind => panic!("{id}: unreviewed native classification {kind}"),
        }

        let cells = case["cells"]
            .as_array()
            .expect("13 named parser obligations");
        assert_eq!(cells.len(), 13, "{id}: no implicit dialect exclusions");
        for (index, cell) in cells.iter().enumerate() {
            let name = cell["dialect"].as_str().expect("dialect identity");
            assert_eq!(cell["dialect"], frozen["dialects"][index]);
            assert_eq!(cell["id"], format!("{id}/{name}"));
            assert!(seen_cells.insert(format!("{id}/{name}")));
            assert_eq!(
                cell["original_observed_status"], source["dialect_cells"][index]["observed_status"],
                "{id}/{name}: frozen parser result changed"
            );
            assert_eq!(cell["vendor_native_execution"], "unattested");
            assert_eq!(cell["independent_semantic_review"], "pending");
        }
    }
    assert_eq!(seen_cases, expected);
    assert_eq!(seen_cells.len(), 494);
    assert_eq!((native_dml, native_ddl, named_boundaries), (16, 18, 4));

    let tracked = &manifest["task_196_schema_write_ddl_acceptance"];
    assert_eq!(tracked["original_cases"], 38);
    assert_eq!(tracked["dialect_cells"], 494);
    assert_eq!(tracked["independent_reviewed_cells"], 494);
    assert_eq!(tracked["reviewed_portable_cells_under_frozen_source"], 402);
    assert_eq!(tracked["reviewed_unapproved_vendor_order_and_cascade_cells"], 26);
    assert_eq!(tracked["reviewed_unapproved_original_parser_and_unsupported_cells"], 66);
    assert_eq!(tracked["task_done"], false);
    assert_eq!(tracked["parent_task88_approved"], false);
    assert_eq!(tracked["release_approved"], false);
}

#[test]
fn original_494_public_cells_retain_explicit_analyzed_unsupported_or_rejected_outcomes() {
    let frozen = read(FREEZE);
    let mut accepted = 0;
    let mut rejected = 0;
    let mut unsupported = 0;
    for case in frozen["cases"].as_array().expect("38 original SQL cases") {
        let id = case["id"].as_str().expect("source variant ID");
        let sql = case["sql"].as_str().expect("original SQL");
        let mut canonical_reference: Option<Value> = None;
        for cell in case["cells"]
            .as_array()
            .expect("13 immutable dialect cells")
        {
            let name = cell["dialect"].as_str().expect("public parser name");
            let dialect = dialect_from_name(name).expect("public parser registry");
            let analysis = analyze_sql(sql, name, dialect.as_ref());
            match cell["original_observed_status"]
                .as_str()
                .expect("classification")
            {
                "R" => {
                    assert!(analysis.is_err(), "{id}/{name}: parse rejection was lost");
                    rejected += 1;
                }
                "U" => {
                    let protocol = analysis.unwrap_or_else(|error| {
                        panic!("{id}/{name}: unsupported must be explicit: {error}")
                    });
                    assert!(
                        protocol.statements().iter().any(|statement| {
                            matches!(statement, ProtocolStatement::Unsupported(_))
                        }),
                        "{id}/{name}: unsupported SQL cannot silently become exact"
                    );
                    unsupported += 1;
                }
                "C" => {
                    let protocol = analysis.unwrap_or_else(|error| {
                        panic!("{id}/{name}: analyzed original SQL regressed: {error}")
                    });
                    assert!(
                        !protocol.statements().is_empty(),
                        "{id}/{name}: SQL statement must not be dropped"
                    );
                    assert!(
                        !protocol.statements().iter().any(|statement| {
                            matches!(statement, ProtocolStatement::Unsupported(_))
                        }),
                        "{id}/{name}: historical analyzed variant became unsupported"
                    );
                    let document = normalized(read(&to_json(&protocol)));
                    if let Some(reference) = &canonical_reference {
                        assert_eq!(
                            &document, reference,
                            "{id}/{name}: complete public document changed across equivalent parsers"
                        );
                    } else {
                        canonical_reference = Some(document);
                    }
                    accepted += 1;
                }
                status => panic!("{id}/{name}: unreviewed classifier {status}"),
            }
        }
    }
    assert_eq!((accepted, rejected, unsupported), (428, 11, 55));
}
