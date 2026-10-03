use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json, ConfiguredSqlInput,
    RelationCatalog, RelationSchema, ScalarType, SchemaColumn, SqlInput,
};

#[test]
fn catalog_source_schema_is_preserved_in_bundle_and_emission() {
    let schema = RelationSchema::new(
        "raw.orders",
        vec![
            SchemaColumn::new("id", ScalarType::Integer).expect("integer column should be valid"),
            SchemaColumn::new("active", ScalarType::Boolean)
                .expect("boolean column should be valid"),
            SchemaColumn::new("created_at", ScalarType::Timestamp)
                .expect("timestamp column should be valid"),
        ],
    )
    .expect("relation schema should be valid");
    let catalog =
        RelationCatalog::from_schemas(std::slice::from_ref(&schema)).expect("catalog should build");
    let input = SqlInput::inline(
        "SELECT id, active, created_at FROM raw.orders WHERE id > 10 AND active = true",
    );
    let dialect = dialect_from_name("generic").expect("generic dialect should exist");
    let configured = [ConfiguredSqlInput::new(
        "orders",
        &input,
        "generic",
        dialect.as_ref(),
    )];

    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("analysis should succeed");

    assert_eq!(bundle.source_schemas(), std::slice::from_ref(&schema));

    let emitted = to_bundle_json(&bundle);
    let value: serde_json::Value =
        serde_json::from_str(&emitted).expect("emitted protocol should be valid JSON");
    assert_eq!(value["source_schemas"][0]["relation"], "raw.orders");
    assert_eq!(value["source_schemas"][0]["columns"][0]["name"], "id");
    assert_eq!(
        value["source_schemas"][0]["columns"][0]["type"],
        "integer"
    );
    assert_eq!(value["source_schemas"][0]["columns"][1]["name"], "active");
    assert_eq!(
        value["source_schemas"][0]["columns"][1]["type"],
        "boolean"
    );
    assert_eq!(
        value["source_schemas"][0]["columns"][2]["name"],
        "created_at"
    );
    assert_eq!(
        value["source_schemas"][0]["columns"][2]["type"],
        "timestamp"
    );
}

#[test]
fn dialect_selection_is_available_without_a_consumer_sqlparser_dependency() {
    assert!(dialect_from_name("postgresql").is_some());
    assert!(dialect_from_name("SNOWFLAKE").is_some());
    assert!(dialect_from_name("not-a-dialect").is_none());
}
