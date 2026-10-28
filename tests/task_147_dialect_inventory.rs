//! TASK-147 finite dialect contract: every frozen SQL cell is tested, never
//! silently skipping a parsed variant or mistaking a parser for a native engine.

use std::collections::BTreeMap;

use sql_semantic_protocol::{analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput};

fn classification(sql: &str, dialect: &str) -> &'static str {
    let parser = dialect_from_name(dialect).expect("all frozen dialects are exposed");
    match analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) {
        Err(_) => "parser_boundary",
        Ok(bundle) => match bundle.inputs()[0].statements().first() {
            Some(ProtocolStatement::Unsupported(_)) => "canonical_unsupported",
            Some(ProtocolStatement::Query(query)) => match query.write() {
                Some(write) if write.insert_conflict().is_some() => "canonical_conflict",
                Some(write) if !write.returning_columns().is_empty() => "canonical_returning",
                Some(_) => "canonical_append",
                None => "canonical_missing_write",
            },
            None => "canonical_missing_write",
            Some(_) => "canonical_unsupported",
        },
    }
}

#[test]
fn every_frozen_dialect_cell_has_an_explicit_verified_parser_classification() {
    let matrix: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-147-acceptance-freeze.json")).unwrap();
    let mut actual = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut seen = std::collections::BTreeSet::new();
    for family in ["mandatory_cases", "conditional_cases"] {
        for case in matrix[family].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            assert!(seen.insert(id), "duplicate frozen variant identity: {id}");
            for dialect in matrix["dialects"].as_array().unwrap() {
                let name = dialect.as_str().unwrap();
                actual.entry(name.to_string()).or_default().insert(
                    id.to_string(),
                    classification(case["sql"].as_str().unwrap(), name).to_string(),
                );
            }
        }
    }
    assert_eq!(matrix["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(seen.len(), 14);
    let expected = &matrix["dialect_cells"];
    assert_eq!(
        serde_json::to_value(&actual).unwrap(),
        *expected,
        "Frozen SQL parser classifications missing or stale. ACTUAL_TASK147_CELLS={}",
        serde_json::to_string(&actual).unwrap()
    );
}
