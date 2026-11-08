//! TASK-177: executable canonical equivalence across all pinned SQL/dialect samples.
//!
//! A SQL parser accepting a query does not prove outcome semantics. This
//! differential test compares the entire public emitted protocol across
//! dialects for the *same* SQL sample. A result with an explicit unsupported
//! statement is never promoted to equivalent. Unknown typed domains remain
//! unknown, even when their protocol shapes agree.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const SNAPSHOT: &str = include_str!("../docs/task-177-parser-snapshot.json");
const EQUIVALENCE: &str = include_str!("../docs/task-177-equivalence-snapshot.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned task evidence")
}

fn rows<'a>(object: &'a Value, key: &str) -> &'a [Value] {
    object[key].as_array().expect("required array")
}

fn normalized_json(protocol: &sql_semantic_protocol::Protocol) -> Value {
    let mut document: Value =
        serde_json::from_str(&to_json(protocol)).expect("public protocol serialization");
    for input in document["inputs"].as_array_mut().expect("one emitted input") {
        input["dialect"] = Value::Null;
    }
    document
}

#[test]
fn every_parsed_variant_has_a_pinned_public_protocol_equivalence_classification() {
    let source = read(SAMPLES);
    let parser = read(SNAPSHOT);
    let pinned = read(EQUIVALENCE);
    assert_eq!(source["schema_version"], 1);
    assert_eq!(pinned["schema_version"], 1);
    assert_eq!(pinned["task_id"], "TASK-177");
    assert_eq!(pinned["dialects"], source["dialects"]);

    let dialects = rows(&source, "dialects");
    let mut observed = BTreeMap::new();
    let mut drift_examples = BTreeMap::new();

    for case in rows(&source, "cases") {
        let id = case["id"].as_str().expect("stable sample ID");
        let sql = case["sql"].as_str().expect("pinned SQL");
        let parser_mask = parser["matrix"][id].as_str().expect("complete parser mask");
        let analyzer_mask = parser["canonical_analysis_matrix"][id]
            .as_str()
            .expect("complete analyzer mask");
        let mut canonical_reference: Option<Value> = None;
        let mut canonical_reference_dialect: Option<String> = None;
        let mut result = String::new();

        for (index, dialect_name) in dialects.iter().enumerate() {
            let dialect_name = dialect_name.as_str().expect("named dialect");
            let parser_status = parser_mask.as_bytes()[index];
            let analyzer_status = analyzer_mask.as_bytes()[index];
            if parser_status == b'R' {
                assert_eq!(analyzer_status, b'R', "{id}/{dialect_name} parser contract");
                result.push('R');
                continue;
            }
            let dialect = dialect_from_name(dialect_name).expect("public SQL dialect");
            let protocol = match analyze_sql(sql, dialect_name, dialect.as_ref()) {
                Err(error) => {
                    assert_eq!(analyzer_status, b'E', "{id}/{dialect_name}: {error}");
                    result.push('E');
                    continue;
                }
                Ok(protocol) => protocol,
            };
            if protocol
                .statements()
                .iter()
                .any(|statement| matches!(statement, ProtocolStatement::Unsupported(_)))
            {
                assert_eq!(analyzer_status, b'U', "{id}/{dialect_name}: unsupported");
                result.push('U');
                continue;
            }
            assert_eq!(analyzer_status, b'A', "{id}/{dialect_name}: not analyzed");
            let canonical = normalized_json(&protocol);
            if let Some(reference) = &canonical_reference {
                if &canonical == reference {
                    result.push('C');
                } else {
                    result.push('D');
                    drift_examples.insert(
                        format!("{id}/{dialect_name}"),
                        canonical_reference_dialect
                            .as_deref()
                            .expect("reference dialect set")
                            .to_owned(),
                    );
                }
            } else {
                canonical_reference = Some(canonical);
                canonical_reference_dialect = Some(dialect_name.to_owned());
                result.push('C');
            }
        }
        assert_eq!(result.len(), 13, "{id}: all parser dialects accounted for");
        observed.insert(id.to_owned(), result);
    }

    let expected = pinned["matrix"].as_object().expect("frozen equivalence matrix");
    if expected.is_empty() {
        panic!(
            "TASK177_EQUIVALENCE_MATRIX={}",
            serde_json::to_string(&observed).expect("sample classification JSON")
        );
    }
    let actual_ids: BTreeSet<_> = observed.keys().map(String::as_str).collect();
    let pinned_ids: BTreeSet<_> = expected.keys().map(String::as_str).collect();
    assert_eq!(actual_ids, pinned_ids, "versioned variant ID drift");
    for (id, mask) in &observed {
        let expected_mask = expected[id].as_str().expect("pinned row");
        assert_eq!(mask, expected_mask, "{id}: full protocol equivalence drift");
        assert!(mask.chars().all(|status| matches!(status, 'C' | 'D' | 'E' | 'U' | 'R')));
    }
    assert_eq!(observed.len(), 251);
    assert_eq!(pinned["native_vendor_execution_certified"], false);
    assert_eq!(pinned["exact_typed_outcome_certified_for_every_variant"], false);
    assert_eq!(pinned["sql_tdg_candidate_pinned"], false);
}
