//! Frozen TASK-178 dialect evidence for every named family variant.
//!
//! This is a complete parser/canonical observation matrix, NOT a semantic
//! proof. Separate task-owned whole-bag/typed oracles must approve each cell.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, ProtocolStatement};

const INVENTORY: &str = include_str!("../docs/task-178-full-variant-inventory.json");
const SAMPLES: &str = include_str!("../docs/task-177-parser-samples.json");
const SNAPSHOT: &str = include_str!("../docs/task-177-equivalence-snapshot.json");
const PARSER: &str = include_str!("../docs/task-177-parser-snapshot.json");
const DRIFTS: &str = include_str!("../docs/task-177-canonical-drift-boundaries.json");
const RECONCILIATION: &str = include_str!("../docs/task-178-canonical-reconciliation.json");

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("frozen task evidence")
}

fn document_without_input_provenance(protocol: &sql_semantic_protocol::Protocol) -> Value {
    let mut value = parse(&to_json(protocol));
    for input in value["inputs"].as_array_mut().expect("public inputs") {
        input["dialect"] = Value::Null;
    }
    value
}

#[test]
fn named_family_inventory_has_no_skips_and_never_promotes_structural_proof() {
    let j = parse(INVENTORY);
    let source = parse(SAMPLES);
    let snapshot = parse(SNAPSHOT);
    let boundaries = parse(DRIFTS);
    assert_eq!(j["schema_version"], 1);
    assert_eq!(j["task_id"], "TASK-178");
    assert_eq!(j["dialects"], source["dialects"]);
    assert_eq!(j["dialects"], snapshot["dialects"]);
    assert_eq!(j["case_count"], 163);
    assert_eq!(j["cell_count"], 2119);
    assert_eq!(j["independently_signed_off_cells"], 0);
    assert_eq!(j["release_approved"], false);

    let source_index: BTreeMap<_, _> = source["cases"]
        .as_array()
        .expect("source inventory")
        .iter()
        .map(|case| (case["id"].as_str().expect("id"), case))
        .collect();
    let mut ids = BTreeSet::new();
    let mut totals = BTreeMap::new();
    let mut named_drift_ids = BTreeSet::new();
    for case in j["cases"].as_array().expect("complete variant records") {
        let id = case["id"].as_str().expect("original variant id");
        assert!(ids.insert(id), "duplicate variant {id}");
        let source = source_index.get(id).expect("original named TASK-66 SQL");
        assert_eq!(
            case["sql"], source["sql"],
            "{id}: fixture SQL must not drift"
        );
        assert_eq!(case["feature_id"], source["feature_id"]);
        assert_eq!(case["variant"], source["variant"]);
        assert_eq!(case["historical_owner"], source["semantic_owner"]);
        let mask = case["full_document_classification"]
            .as_str()
            .expect("13 observed statuses");
        assert_eq!(mask.len(), 13, "{id}: 13 dialects, no implicit skips");
        assert_eq!(case["full_document_classification"], snapshot["matrix"][id]);
        let cells = case["dialect_cells"].as_array().expect("individual cells");
        assert_eq!(cells.len(), 13, "{id}: full evidence grid");
        for (i, cell) in cells.iter().enumerate() {
            assert_eq!(cell["dialect"], j["dialects"][i]);
            assert_eq!(
                cell["observed_status"].as_str().expect("status"),
                &mask[i..=i]
            );
            assert_eq!(
                cell["typed_membership"], "pending_independent_equivalent_source_terminal_evidence",
                "{id}: full-domain membership remains unproved"
            );
            assert_eq!(cell["vendor_native_execution"], "unattested");
            let status = mask.as_bytes()[i] as char;
            *totals.entry(status).or_insert(0_usize) += 1;
            if status == 'D' {
                let dialect = cell["dialect"].as_str().expect("dialect");
                named_drift_ids.insert(format!("{id}-{dialect}"));
            }
        }
    }
    assert_eq!(totals.get(&'C'), Some(&2014));
    assert_eq!(totals.get(&'D'), Some(&23));
    assert_eq!(totals.get(&'U'), Some(&55));
    assert_eq!(totals.get(&'R'), Some(&27));
    let expected_drift: BTreeSet<_> = boundaries["cells"]
        .as_array()
        .expect("known drifts")
        .iter()
        .map(|cell| cell["id"].as_str().expect("drift cell").to_string())
        .collect();
    assert_eq!(
        named_drift_ids, expected_drift,
        "no unowned normalized-public-document drift"
    );
    assert_eq!(ids.len(), 163);
}

#[test]
fn actual_full_public_protocol_matches_each_frozen_observed_dialect_classification() {
    let inventory = parse(INVENTORY);
    let parsed = parse(PARSER);
    let snapshot = parse(SNAPSHOT);
    let reconciliation = parse(RECONCILIATION);
    let fixed: BTreeSet<_> = reconciliation["resolved_grouping_cells"]
        .as_array()
        .expect("exactly 22 reviewed normalization candidates")
        .iter()
        .map(|cell| {
            format!(
                "{}/{}",
                cell["case_id"].as_str().expect("case"),
                cell["dialect"].as_str().expect("dialect")
            )
        })
        .collect();
    assert_eq!(fixed.len(), 22);
    for case in inventory["cases"].as_array().expect("variant cases") {
        let id = case["id"].as_str().expect("stable case id");
        let sql = case["sql"].as_str().expect("original SQL text");
        let observed = case["full_document_classification"]
            .as_str()
            .expect("frozen classifications");
        let mut baseline: Option<Value> = None;
        for (index, dialect_name) in inventory["dialects"]
            .as_array()
            .expect("13 names")
            .iter()
            .enumerate()
        {
            let name = dialect_name.as_str().expect("dialect name");
            let original_parser = parsed["matrix"][id].as_str().expect("parser statuses");
            let original_analyzer = parsed["canonical_analysis_matrix"][id]
                .as_str()
                .expect("analyzer statuses");
            // Always execute the current parser, including historically rejected
            // cells. Otherwise a parser regression can silently retain a stale
            // 'R' classification without running the dialect at all.
            let dialect = dialect_from_name(name).expect("exposed SQL dialect");
            let parser_rejected = original_parser.as_bytes()[index] == b'R';
            let actual = match analyze_sql(sql, name, dialect.as_ref()) {
                Err(sql_semantic_protocol::Error::Parse(_)) if parser_rejected => {
                    assert_eq!(original_analyzer.as_bytes()[index], b'R');
                    'R'
                }
                Err(error) => panic!("{id}/{name}: unexpected parser/analyzer error {error}"),
                Ok(_) if parser_rejected => {
                    panic!("{id}/{name}: previously rejected SQL now parses; update the reviewed dialect boundary")
                }
                Ok(protocol)
                    if protocol.statements().iter().any(|statement| {
                        matches!(statement, ProtocolStatement::Unsupported(_))
                    }) =>
                {
                    'U'
                }
                Ok(protocol) => {
                    let next = document_without_input_provenance(&protocol);
                    if let Some(expected) = &baseline {
                        if &next == expected {
                            'C'
                        } else {
                            'D'
                        }
                    } else {
                        baseline = Some(next);
                        'C'
                    }
                }
            };
            let historical = observed.as_bytes()[index];
            assert_eq!(
                historical,
                snapshot["matrix"][id]
                    .as_str()
                    .expect("immutable TASK-177 classification")
                    .as_bytes()[index]
            );
            let expected = if fixed.contains(&format!("{id}/{name}")) {
                assert_eq!(historical, b'D', "only historical drift can be repaired");
                b'C'
            } else {
                historical
            };
            assert_eq!(
                actual as u8, expected,
                "{id}/{name}: current public protocol must obey the explicit TASK-178 overlay"
            );
        }
    }
}
