//! TASK-180: independently execute the current public analyzer against every
//! original 163-by-13 frozen SQL/dialect cell. Historical observations and
//! reviewed normalization fixes remain separate from native engine evidence.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sql_semantic_protocol::{analyze_sql, dialect_from_name, to_json, Error, ProtocolStatement};

const FREEZE: &str = include_str!("../docs/task-180-acceptance-freeze.json");
const RECONCILIATION: &str = include_str!("../docs/task-178-canonical-reconciliation.json");
const GAP_LEDGER: &str = include_str!("../docs/task-180-exception-and-gap-ledger.json");
const VENDOR_MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");

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
                    if protocol.statements().iter().any(|statement| {
                        matches!(statement, ProtocolStatement::Unsupported(_))
                    }) =>
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
    assert_eq!(
        seen.len(),
        2119,
        "every mandatory SQL/dialect cell is exercised"
    );
    assert_eq!(actual_counts.get(&'C'), Some(&2036));
    assert_eq!(actual_counts.get(&'D'), Some(&1));
    assert_eq!(actual_counts.get(&'U'), Some(&55));
    assert_eq!(actual_counts.get(&'R'), Some(&27));
}

#[test]
fn original_dialect_exceptions_have_a_complete_actionable_gap_ledger() {
    let freeze = read(FREEZE);
    let gaps = read(GAP_LEDGER);
    let reconciliation = read(RECONCILIATION);
    assert_eq!(gaps["schema_version"], 1);
    assert_eq!(gaps["task_id"], "TASK-180");
    assert_eq!(gaps["original_native_review"]["exception_count"], 23);
    assert_eq!(
        gaps["independent_bounded_adapter_review"]["certified"],
        1861
    );
    assert_eq!(
        gaps["independent_bounded_adapter_review"]["conditional"],
        152
    );
    assert_eq!(
        gaps["independent_bounded_adapter_review"]["unrepresentable"],
        106
    );
    assert_eq!(gaps["vendor_session_laws"]["reviewed_cells"], 481);
    assert_eq!(
        gaps["vendor_session_laws"]["external_native_vendor_attested"],
        false
    );
    assert_eq!(gaps["policy"]["task180_done"], false);

    let normalized: BTreeSet<String> = reconciliation["resolved_grouping_cells"]
        .as_array()
        .expect("reviewed original canonical normalization")
        .iter()
        .map(|item| {
            format!(
                "{}/{}",
                item["case_id"].as_str().expect("original variant"),
                item["dialect"].as_str().expect("reviewed dialect"),
            )
        })
        .collect();
    assert_eq!(normalized.len(), 22);

    let expected: BTreeMap<_, _> = freeze["cases"]
        .as_array()
        .expect("original SQL")
        .iter()
        .flat_map(|case| case["cells"].as_array().expect("13 cells"))
        .filter(|cell| cell["original_canonical_class"] != "C")
        .map(|cell| {
            (
                cell["id"].as_str().expect("stable cell").to_string(),
                cell["original_canonical_class"]
                    .as_str()
                    .expect("old parser/canonical observation")
                    .to_string(),
            )
        })
        .collect();
    assert_eq!(
        expected.len(),
        105,
        "one ledger entry for every unusual original cell"
    );

    let mut seen = BTreeSet::new();
    let mut resolved = 0;
    let mut still_divergent = 0;
    for case in gaps["original_exception_cases"]
        .as_array()
        .expect("original families")
    {
        for cell in case["cells"].as_array().expect("named exception cells") {
            let key = cell["id"].as_str().expect("stable original ID/dialect");
            assert!(seen.insert(key.to_string()), "{key}: duplicate exception");
            assert_eq!(
                cell["original_class"],
                expected
                    .get(key)
                    .expect("exact original non-C observation")
                    .as_str()
            );
            if normalized.contains(key) {
                resolved += 1;
                assert_eq!(
                    cell["required_gate"],
                    "historical_canonical_drift_normalized"
                );
                assert_eq!(
                    cell["expected_release_state"],
                    "portable_candidate_pending_final_review"
                );
            } else if cell["original_class"] == "D" {
                still_divergent += 1;
                assert_eq!(cell["required_gate"], "canonical_divergence_unapproved");
            } else {
                assert_eq!(
                    cell["expected_release_state"],
                    "unapproved_pending_independent_evidence"
                );
            }
        }
    }
    assert_eq!(seen, expected.keys().cloned().collect());
    assert_eq!((resolved, still_divergent), (22, 1));
    assert_eq!(gaps["historical_canonical_counts"]["original_C"], 2014);
    assert_eq!(gaps["historical_canonical_counts"]["original_D"], 23);
    assert_eq!(gaps["historical_canonical_counts"]["original_U"], 55);
    assert_eq!(gaps["historical_canonical_counts"]["original_R"], 27);
}

#[test]
fn all_481_original_vendor_session_law_cells_have_live_fail_closed_boundaries() {
    let native = read(VENDOR_MATRIX);
    let full = read(FREEZE);
    assert_eq!(native["case_count"], 37);
    assert_eq!(native["dialect_cell_count"], 481);
    assert_eq!(native["dialects"], full["dialects"]);
    assert_eq!(native["external_vendor_runtime_proven"], false);

    let mut seen = BTreeSet::new();
    let mut observed = [0_usize; 3];
    for case in native["cases"].as_array().expect("37 frozen vendor laws") {
        let id = case["id"].as_str().expect("stable vendor law");
        let sql = case["sql"].as_str().expect("actual vendor SQL");
        let cells = case["dialect_cells"].as_array().expect("all 13 named dialects");
        assert_eq!(cells.len(), 13, "{id}: absent vendor cell");
        for (index, cell) in cells.iter().enumerate() {
            let name = full["dialects"][index].as_str().expect("named parser");
            let key = format!("{id}/{name}");
            assert!(seen.insert(key.clone()), "{key}: duplicate vendor law cell");
            assert_eq!(cell["dialect"], name);
            assert_eq!(cell["native_vendor"], "unattested");
            assert_eq!(cell["conditional_owner"], "TASK-179");
            assert_eq!(cell["release_disposition"], "unapproved_vendor_law");
            assert_eq!(
                cell["review_status"],
                "reviewed_bounded_source_semantics_with_explicit_vendor_residual"
            );

            let parser = dialect_from_name(name).expect("project dialect");
            match analyze_sql(sql, name, parser.as_ref()) {
                Err(Error::Parse(_)) => {
                    assert_eq!(cell["parser_status"], "rejected", "{key}: original parser boundary");
                    assert_eq!(cell["analyzer_status"], "parser_rejected");
                    observed[2] += 1;
                }
                Err(error) => panic!("{key}: unreviewed vendor analyzer error: {error}"),
                Ok(protocol)
                    if protocol.statements().iter().any(|statement| {
                        matches!(statement, ProtocolStatement::Unsupported(_))
                    }) =>
                {
                    assert_eq!(cell["parser_status"], "parsed");
                    assert_eq!(cell["analyzer_status"], "unsupported", "{key}");
                    observed[1] += 1;
                }
                Ok(protocol) => {
                    assert_eq!(cell["parser_status"], "parsed");
                    assert_eq!(cell["analyzer_status"], "analyzed", "{key}");
                    let wire = read(&to_json(&protocol));
                    assert!(
                        wire["inputs"].is_array(),
                        "{key}: parsed vendor syntax must emit a canonical protocol"
                    );
                    observed[0] += 1;
                }
            }
        }
    }
    assert_eq!(seen.len(), 481);
    assert_eq!(observed, [442, 35, 4]);
}
