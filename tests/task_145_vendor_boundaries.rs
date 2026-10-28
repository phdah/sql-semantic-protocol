//! TASK-145 vendor-sensitive INSERT syntax is never silently promoted to append.

use sql_semantic_protocol::{analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput};

#[test]
fn vendor_insert_modifiers_require_distinct_semantics_or_parser_boundary() {
    let matrix: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-145-acceptance-freeze.json")).unwrap();
    let vendor_cases = matrix["vendor_syntax_boundaries"].as_array().unwrap();
    let mut checked = 0;
    for case in vendor_cases {
        assert_eq!(case["owner"], "TASK-145");
        assert_eq!(case["typed_proof"], "Residual");
        let dialect = case["dialect"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let parser = dialect_from_name(dialect).unwrap();
        // Parser rejection is not evidence of native runtime behavior.
        if let Ok(bundle) = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref()) {
            let statement = &bundle.inputs()[0].statements()[0];
            // TASK-147 now normalizes these two bounded forms. The legacy
            // TASK-145 vendor case still has Residual physical proof until
            // the caller supplies attested target key/source state evidence.
            // All other native modifiers must remain explicit unsupported.
            match case["id"].as_str().unwrap() {
                "80-C145-V4" => match statement {
                    ProtocolStatement::Query(query) => {
                        let write = query.write().expect("conflict INSERT");
                        assert_eq!(
                            write.insert_conflict().expect("targeted DO NOTHING").columns(),
                            &["id"]
                        );
                        assert!(write.returning_columns().is_empty());
                    }
                    _ => panic!("bounded ON CONFLICT must retain policy: {statement:?}"),
                },
                "80-C145-V5" => match statement {
                    ProtocolStatement::Query(query) => {
                        let write = query.write().expect("RETURNING INSERT");
                        assert!(write.insert_conflict().is_none());
                        assert_eq!(write.returning_columns(), &["id"]);
                    }
                    _ => panic!("direct RETURNING must retain projection: {statement:?}"),
                },
                _ => assert!(
                    matches!(statement, ProtocolStatement::Unsupported(_)),
                    "{dialect}: unsupported modifier cannot become ordinary append: {sql}: {statement:?}"
                ),
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 7);
}
