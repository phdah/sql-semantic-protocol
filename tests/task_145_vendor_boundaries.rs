//! TASK-145 vendor-sensitive INSERT syntax is never silently promoted to append.

use sql_semantic_protocol::{analyze_inputs, dialect_from_name, ProtocolStatement, SqlInput};

#[test]
fn vendor_insert_modifiers_require_distinct_semantics_or_parser_boundary() {
    let vendor_cases = [
        ("sqlite", "INSERT OR IGNORE INTO target (id,score) VALUES (1,10)"),
        ("mysql", "REPLACE INTO target (id,score) VALUES (1,10)"),
        ("hive", "INSERT OVERWRITE TABLE target VALUES (1,10)"),
        ("postgresql", "INSERT INTO target (id,score) VALUES (1,10) ON CONFLICT(id) DO NOTHING"),
        ("postgresql", "INSERT INTO target (id,score) VALUES (1,10) RETURNING id"),
        ("mysql", "INSERT INTO target SET id = 1, score = 10"),
        ("mysql", "INSERT INTO target (id,score) VALUES (1,10) ON DUPLICATE KEY UPDATE score=11"),
    ];
    let mut checked = 0;
    for (dialect, sql) in vendor_cases {
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
