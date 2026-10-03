use sql_semantic_protocol::{
    ConfiguredSqlInput, RelationCatalog, RelationSchema, ScalarType, SchemaColumn, SqlInput,
    analyze_configured_inputs_with_catalog, dialect_from_name, to_bundle_json,
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
    assert!(emitted.contains(
        r#""source_schemas":[{"relation":"raw.orders","columns":[{"name":"id","type":"integer"},{"name":"active","type":"boolean"},{"name":"created_at","type":"timestamp"}]}]"#
    ));
}

#[test]
fn dialect_selection_is_available_without_a_consumer_sqlparser_dependency() {
    assert!(dialect_from_name("postgresql").is_some());
    assert!(dialect_from_name("SNOWFLAKE").is_some());
    assert!(dialect_from_name("not-a-dialect").is_none());
}
