//! Physical comparison laws must survive the public protocol serialization boundary.

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json, ConfiguredSqlInput,
    ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
    DataType, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, TextComparisonLaw,
};

#[test]
fn binary_text_law_and_its_authoritative_provenance_are_serialized() {
    let evidence = ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "warehouse:binary")
            .unwrap(),
        ConstraintEnforcement::Enforced,
    );
    let attested = SchemaColumn::new(
        "id",
        DataType::String {
            length: Some(12),
            fixed: false,
        },
    )
    .unwrap()
    .with_collation("warehouse_binary")
    .unwrap()
    .with_text_comparison_law(TextComparisonLaw::Binary, vec![evidence])
    .unwrap();
    let unattested = SchemaColumn::new(
        "description",
        DataType::String {
            length: None,
            fixed: false,
        },
    )
    .unwrap()
    .with_collation("unknown")
    .unwrap();
    let schema = RelationSchema::new("target", vec![attested, unattested]).unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline("SELECT id FROM target");
    let input = ConfiguredSqlInput::new("test", &sql, "duckdb", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();

    let json: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    let columns = json["source_schemas"][0]["columns"].as_array().unwrap();
    assert_eq!(columns[0]["text_comparison_law"], "binary");
    assert_eq!(columns[0]["collation"], "warehouse_binary");
    assert_eq!(
        columns[0]["comparison_evidence"],
        serde_json::json!([{
            "source_kind": "external_metadata",
            "source_id": "warehouse:binary",
            "enforcement": "enforced"
        }])
    );
    assert_eq!(columns[1]["collation"], "unknown");
    assert!(columns[1].get("text_comparison_law").is_none());
    assert!(columns[1].get("comparison_evidence").is_none());

    // A consumer must not have to infer the comparison law from the collation name.
    assert!(SchemaColumn::new(
        "id",
        DataType::String {
            length: None,
            fixed: false
        },
    )
    .unwrap()
    .with_text_comparison_law(TextComparisonLaw::Binary, vec![])
    .is_err());
}
