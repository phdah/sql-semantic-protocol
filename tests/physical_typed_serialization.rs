//! Typed physical witness sidecars preserve physical values without SQL casts.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    to_physical_typed_population_json, ConfiguredSqlInput, ConstraintEnforcement,
    ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind, DataType,
    DataTypeField, LiteralValue, PhysicalTypedPopulationPlan, PhysicalTypedValue,
    PhysicalTypedWriteRow, PhysicalTypedWriteState, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput,
};

fn bundle() -> sql_semantic_protocol::AnalysisBundle {
    let fields = vec![
        ("id", DataType::SignedInteger { bits: Some(32) }),
        ("raw", DataType::Binary { length: Some(3), fixed: false }),
        ("bits", DataType::BitString { length: Some(3) }),
        ("floating", DataType::FloatingPoint { bits: Some(64) }),
        ("document", DataType::Json),
        ("optional", DataType::Nullable(Box::new(DataType::String {
            length: Some(8),
            fixed: false,
        }))),
        ("payload", DataType::Struct {
            fields: vec![DataTypeField::new(
                Some("series".into()),
                DataType::Array {
                    element: Some(Box::new(DataType::SignedInteger { bits: Some(32) })),
                    length: Some(2),
                },
            )],
        }),
    ];
    let schema = RelationSchema::new(
        "target",
        fields.into_iter().map(|(name,kind)| SchemaColumn::new(name, kind).unwrap()).collect(),
    ).unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let sql = SqlInput::inline("SELECT id FROM target");
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = ConfiguredSqlInput::new("typed-wire", &sql, "duckdb", dialect.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[input], &catalog).unwrap();
    let attested = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata,"warehouse").unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    bundle.enrich_relation_constraints(&[
        RelationConstraintSet::new(
            "target",
            vec![RelationConstraint::not_null("document",attested).unwrap()],
        ).unwrap(),
    ]);
    bundle
}

fn state() -> PhysicalTypedWriteState {
    let values = BTreeMap::from([
        ("id".into(), PhysicalTypedValue::Scalar(LiteralValue::Number("1".into()))),
        ("raw".into(), PhysicalTypedValue::Bytes(vec![0, 255, 2])),
        ("bits".into(), PhysicalTypedValue::Bits(vec![true, false, true])),
        ("floating".into(), PhysicalTypedValue::Float64Bits(1.5_f64.to_bits())),
        ("document".into(), PhysicalTypedValue::Json(serde_json::json!({"nullable":null,"count":7}))),
        ("optional".into(), PhysicalTypedValue::Scalar(LiteralValue::Null)),
        ("payload".into(), PhysicalTypedValue::Struct(vec![PhysicalTypedValue::Array(vec![
            PhysicalTypedValue::Scalar(LiteralValue::Number("7".into())),
            PhysicalTypedValue::Scalar(LiteralValue::Number("8".into())),
        ])])),
    ]);
    PhysicalTypedWriteState::new(
        "target",
        vec![PhysicalTypedWriteRow::new(values).unwrap()],
    ).unwrap()
}

#[test]
fn validated_typed_witness_sidecar_preserves_bits_bytes_json_null_and_order() {
    let bundle = bundle();
    let text = to_physical_typed_population_json(&bundle, &[state()]).unwrap();
    assert_eq!(text, to_physical_typed_population_json(&bundle, &[state()]).unwrap());
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["kind"], "typed_physical_population");
    assert_eq!(parsed["protocol_version"], env!("CARGO_PKG_VERSION"));
    let row = &parsed["populations"][0]["rows"][0]["values"];
    assert_eq!(row["raw"], serde_json::json!({"kind":"bytes","values":[0,255,2]}));
    assert_eq!(row["bits"], serde_json::json!({"kind":"bits","values":[true,false,true]}));
    assert_eq!(row["floating"],serde_json::json!({
        "kind":"float64_bits","bits":1.5_f64.to_bits(),
    }));
    assert_eq!(row["document"]["kind"], "json_document");
    assert!(row["document"]["document"]["nullable"].is_null());
    assert_eq!(row["optional"]["scalar_kind"], "null");
    assert_eq!(row["payload"]["values"][0]["values"][1]["value"], "8");

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../schema/typed-physical-population.schema.json")).unwrap();
    assert_eq!(schema["$defs"]["value"]["oneOf"].as_array().unwrap().len(), 13);
}

#[test]
fn invalid_or_unproved_typed_rows_cannot_be_serialized_as_proved_witnesses() {
    let mut invalid = state();
    let mut values = invalid.rows()[0].values().clone();
    values.insert("document".into(),PhysicalTypedValue::Scalar(LiteralValue::Null));
    invalid = PhysicalTypedWriteState::new("target",vec![PhysicalTypedWriteRow::new(values).unwrap()]).unwrap();
    assert!(matches!(
        to_physical_typed_population_json(&bundle(), &[invalid]),
        Err(PhysicalTypedPopulationPlan::Impossible { .. })
    ));

    let mut values = state().rows()[0].values().clone();
    values.insert("floating".into(),PhysicalTypedValue::Float64Bits(f64::NAN.to_bits()));
    let residual = PhysicalTypedWriteState::new(
        "target",vec![PhysicalTypedWriteRow::new(values).unwrap()]
    ).unwrap();
    assert!(matches!(
        to_physical_typed_population_json(&bundle(), &[residual]),
        Err(PhysicalTypedPopulationPlan::Residual { .. })
    ));
}
