mod common;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, parse_data_type, to_bundle_json,
    ConfiguredSqlInput, DataType, DataTypeField, RelationCatalog, RelationSchema, SchemaColumn,
    SqlInput,
};

#[test]
fn every_exposed_dialect_normalizes_shared_boolean_type() {
    for dialect in common::DIALECTS {
        assert_eq!(
            parse_data_type("BOOLEAN", dialect),
            Ok(DataType::Boolean),
            "dialect {dialect}"
        );
    }
}

#[test]
fn dialect_specific_types_normalize_to_common_semantics() {
    let cases = [
        ("mysql", "JSON", DataType::Json),
        ("postgresql", "JSONB", DataType::Json),
        ("snowflake", "VARIANT", DataType::Json),
        ("redshift", "SUPER", DataType::Json),
        (
            "mssql",
            "NVARCHAR(MAX)",
            DataType::String {
                length: None,
                fixed: false,
            },
        ),
        (
            "clickhouse",
            "Array(UInt64)",
            DataType::Array {
                element: Some(Box::new(DataType::UnsignedInteger { bits: Some(64) })),
                length: None,
            },
        ),
        (
            "bigquery",
            "ARRAY<INT64>",
            DataType::Array {
                element: Some(Box::new(DataType::SignedInteger { bits: Some(64) })),
                length: None,
            },
        ),
        (
            "databricks",
            "TIMESTAMP_NTZ",
            DataType::Timestamp { precision: None },
        ),
        (
            "ansi",
            "TIMESTAMP WITH TIME ZONE",
            DataType::Timestamp { precision: None },
        ),
        (
            "sqlite",
            "BLOB",
            DataType::Binary {
                length: None,
                fixed: false,
            },
        ),
    ];

    for (dialect, sql_type, expected) in cases {
        assert_eq!(
            parse_data_type(sql_type, dialect),
            Ok(expected),
            "{dialect} {sql_type}"
        );
    }
}

#[test]
fn complex_types_are_recursive_and_parser_independent() {
    assert_eq!(
        parse_data_type("STRUCT<a STRING, b ARRAY<INT64>>", "bigquery"),
        Ok(DataType::Struct {
            fields: vec![
                DataTypeField::new(
                    Some("a".to_owned()),
                    DataType::String {
                        length: None,
                        fixed: false,
                    },
                ),
                DataTypeField::new(
                    Some("b".to_owned()),
                    DataType::Array {
                        element: Some(Box::new(DataType::SignedInteger { bits: Some(64) })),
                        length: None,
                    },
                ),
            ],
        })
    );
}

#[test]
fn catalog_source_schema_is_preserved_in_bundle_and_emission() {
    let schema = RelationSchema::new(
        "raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql")
                .expect("integer column should be valid"),
            SchemaColumn::from_sql_type("payload", "JSONB", "postgresql")
                .expect("JSON column should be valid"),
            SchemaColumn::from_sql_type("created_at", "TIMESTAMPTZ", "postgresql")
                .expect("timestamp column should be valid"),
        ],
    )
    .expect("relation schema should be valid");
    let catalog =
        RelationCatalog::from_schemas(std::slice::from_ref(&schema)).expect("catalog should build");
    let input = SqlInput::inline("SELECT id, payload, created_at FROM raw.orders WHERE id > 10");
    let dialect = dialect_from_name("postgresql").expect("PostgreSQL dialect should exist");
    let configured = [ConfiguredSqlInput::new(
        "orders",
        &input,
        "postgresql",
        dialect.as_ref(),
    )];

    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("analysis should succeed");

    assert_eq!(bundle.source_schemas(), std::slice::from_ref(&schema));

    let emitted = to_bundle_json(&bundle);
    assert!(emitted.contains(r#""kind":"signed_integer""#));
    assert!(emitted.contains(r#""kind":"json""#));
    assert!(emitted.contains(r#""kind":"timestamp""#));
}

#[test]
fn dialect_selection_is_available_without_a_consumer_sqlparser_dependency() {
    assert!(dialect_from_name("postgresql").is_some());
    assert!(dialect_from_name("SNOWFLAKE").is_some());
    assert!(dialect_from_name("not-a-dialect").is_none());
}
