//! TASK-180: independently execute the current public analyzer against every
//! original 163-by-13 frozen SQL/dialect cell. Historical observations and
//! reviewed normalization fixes remain separate from native engine evidence.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, Error, ProtocolStatement};

const FREEZE: &str = include_str!("../docs/task-180-acceptance-freeze.json");
const RECONCILIATION: &str = include_str!("../docs/task-178-canonical-reconciliation.json");

fn read(source: &str) -> Value {
    serde_json::from_str(source).expect("versioned SQL and dialect evidence")
}

fn without_parser_provenance(protocol: &sql_semantic_protocol::Protocol) -> Value {
    let mut document = read(&to_json(protocol));
    for input in document["inputs"].as_array_mut().expect("protocol inputs") {
        input["dialect"] = Value::Null;
    }
    document
}

#[test]
fn all_2119_original_public_sql_dialect_cells_are_executed_with_reviewed_boundaries() {
    let freeze = read(FREEZE);
    let reconciliation = read(RECONCILIATION);
    let fixed: BTreeSet<_> = reconciliation["resolved_grouping_cells"]
        .as_array()
        .expect("reviewed AST normalization fixes")
        .iter()
        .map(|cell| {
            format!(
                "{}/{}",
                cell["case_id"].as_str().expect("case ID"),
                cell["dialect"].as_str().expect("dialect identity")
            )
        })
        .collect();
    assert_eq!(fixed.len(), 22);

    let mut actual_counts = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for case in freeze["cases"].as_array().expect("all original SQLs") {
        let id = case["id"].as_str().expect("stable original SQL ID");
        let sql = case["sql"].as_str().expect("original SQL");
        let mut baseline: Option<Value> = None;
        for cell in case["cells"].as_array().expect("13 parser cells") {
            let name = cell["dialect"].as_str().expect("public parser dialect");
            let cell_id = format!("{id}/{name}");
            assert!(seen.insert(cell_id.clone()), "{cell_id}: duplicate case");
            let parser = dialect_from_name(name).expect("supported parser");
            let actual = match analyze_sql(sql, name, parser.as_ref()) {
                Err(Error::Parse(_)) => 'R',
                Err(error) => panic!("{cell_id}: unreviewed error: {error}"),
                Ok(protocol)
                    if protocol
                        .statements()
                        .iter()
                        .any(|statement| matches!(statement, ProtocolStatement::Unsupported(_))) =>
                {
                    'U'
                }
                Ok(protocol) => {
                    let document = without_parser_provenance(&protocol);
                    let result = match &baseline {
                        Some(previous) if previous != &document => 'D',
                        _ => 'C',
                    };
                    if baseline.is_none() {
                        baseline = Some(document);
                    }
                    result
                }
            };
            let old = cell["original_canonical_class"]
                .as_str()
                .expect("historical parser/canonical status");
            let expected = if fixed.contains(&cell_id) {
                assert_eq!(old, "D", "{cell_id}: only reviewed drift may normalize");
                'C'
            } else {
                old.chars().next().expect("one status marker")
            };
            assert_eq!(
                actual, expected,
                "{cell_id}: live public protocol differs from frozen reviewed expectations"
            );
            *actual_counts.entry(actual).or_insert(0_usize) += 1;
        }
    }
    assert_eq!(seen.len(), 2119, "every mandatory SQL/dialect cell is exercised");
    assert_eq!(actual_counts.get(&'C'), Some(&2036));
    assert_eq!(actual_counts.get(&'D'), Some(&1));
    assert_eq!(actual_counts.get(&'U'), Some(&55));
    assert_eq!(actual_counts.get(&'R'), Some(&27));
}
