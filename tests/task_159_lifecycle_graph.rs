//! TASK-159 canonical lifecycle graph producer identity contracts.

use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, to_bundle_json, SqlInput, WriteKind,
};

fn analyze(sql: &str) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name("duckdb").unwrap();
    analyze_inputs(&[SqlInput::inline(sql)], "duckdb", parser.as_ref()).unwrap()
}

#[test]
fn rename_consumes_original_relation_and_produces_only_new_identity() {
    let bundle = analyze("ALTER TABLE t RENAME TO u");
    let [layer] = bundle.layers() else {
        panic!("one lifecycle layer");
    };
    assert_eq!(layer.write_kind(), Some(WriteKind::Lifecycle));
    assert_eq!(layer.consumes(), &["t"]);
    assert_eq!(
        layer
            .produces()
            .iter()
            .map(|r| r.relation_name())
            .collect::<Vec<_>>(),
        vec![Some("u")],
    );
    let wire: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    let query = &wire["inputs"][0]["statements"][0];
    assert_eq!(wire["layers"][0]["produces"][0]["name"], "u");
    assert_eq!(query["write"]["target"], "t");
    assert_eq!(query["write"]["lifecycle_action"]["to"], "u");
}

#[test]
fn drop_is_destructive_not_a_phantom_anonymous_or_named_producer() {
    let bundle = analyze("DROP TABLE t");
    let [layer] = bundle.layers() else { panic!("one lifecycle layer"); };
    assert_eq!(layer.write_kind(), Some(WriteKind::Lifecycle));
    assert_eq!(layer.consumes(), &["t"]);
    assert!(layer.produces().is_empty(), "DROP is not a relation definition");
    let wire: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    assert_eq!(wire["layers"][0]["produces"], serde_json::json!([]));
}

#[test]
fn truncate_preserves_named_producer_identity_but_not_old_rows() {
    let bundle = analyze("TRUNCATE TABLE t");
    let [layer] = bundle.layers() else {
        panic!("one lifecycle layer");
    };
    assert_eq!(layer.write_kind(), Some(WriteKind::Lifecycle));
    assert_eq!(layer.consumes(), &["t"]);
    assert_eq!(layer.produces()[0].relation_name(), Some("t"));
}
