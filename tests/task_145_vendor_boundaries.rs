//! TASK-145 vendor-sensitive INSERT syntax is never silently promoted to append.

use sql_semantic_protocol::{analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput};

#[test]
fn vendor_insert_modifiers_require_distinct_semantics_or_parser_boundary() {
    let matrix: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-145-acceptance-freeze.json")).unwrap();
    let vendor_cases=matrix["vendor_syntax_boundaries"].as_array().unwrap();
    let mut checked = 0;
    for case in vendor_cases {
        assert_eq!(case["owner"],"TASK-145");
        assert_eq!(case["typed_proof"],"Residual");
        let dialect=case["dialect"].as_str().unwrap();
        let sql=case["sql"].as_str().unwrap();
        let parser = dialect_from_name(dialect).unwrap();
        // Parser rejection is not evidence of native runtime behavior.
        if let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) {
            let statement = &bundle.inputs()[0].statements()[0];
            assert!(
                matches!(statement, ProtocolStatement::Unsupported(_)),
                "{dialect}: modifier must not be represented as a plain INSERT append: {sql}: {statement:?}"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 7);
}
