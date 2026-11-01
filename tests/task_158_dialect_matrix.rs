//! TASK-158 explicit SQL parser-boundary and canonical ALTER classification matrix.
//! No parser acceptance is mistaken for an engine execution or cast proof.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput, WriteKind,
};
use sqlparser::parser::Parser;
use std::collections::BTreeMap;

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-158-acceptance-freeze.json")).unwrap()
}

#[test]
fn every_frozen_alter_variant_has_a_classification_in_all_thirteen_dialects() {
    let f = freeze();
    let mut observed = BTreeMap::<String, Vec<&'static str>>::new();
    let mut cells = 0;
    for dialect_name in f["dialects"].as_array().unwrap() {
        let name = dialect_name.as_str().unwrap();
        let parser = dialect_from_name(name).unwrap();
        let mut results = Vec::new();
        for case in f["cases"].as_array().unwrap() {
            let sql = case["sql"].as_str().unwrap();
            cells += 1;
            if Parser::parse_sql(parser.as_ref(), sql).is_err() {
                results.push("parser_boundary");
                continue;
            }
            let result = analyze_inputs(&[SqlInput::inline(sql)], name, parser.as_ref()).unwrap();
            let status = match &result.inputs()[0].statements()[0] {
                ProtocolStatement::Query(q)
                    if q.write().is_some_and(|write| {
                        write.kind() == WriteKind::SchemaAlter
                            && write.column_alter_steps().len() == 1
                    }) =>
                {
                    "canonical_alter"
                }
                ProtocolStatement::Unsupported(_) => "canonical_unsupported",
                _ => "unexpected_canonical_shape",
            };
            results.push(status);
        }
        observed.insert(name.to_owned(), results);
    }
    assert_eq!(cells, 13 * f["cases"].as_array().unwrap().len());
    assert_eq!(
        serde_json::to_value(observed).unwrap(),
        f["dialect_expected"],
        "Every parser/canonical matrix cell must be frozen independently"
    );
}
