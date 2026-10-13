mod common;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name, parse_data_type,
    to_bundle_json, ColumnValueKind, ConfiguredSqlInput, DataType, DataTypeField,
    GeneratedValueExpression, RelationCatalog, RelationSchema, SchemaColumn, SchemaSourceKind,
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

#[test]
fn physical_collation_is_retained_as_identity_without_inferred_comparison_law() {
    let column = SchemaColumn::from_sql_type("name", "VARCHAR(20)", "postgresql")
        .unwrap()
        .with_collation("sv_SE.utf8")
        .unwrap();
    assert_eq!(column.collation(), Some("sv_SE.utf8"));
    assert!(SchemaColumn::from_sql_type("id", "BIGINT", "postgresql")
        .unwrap()
        .with_collation("sv_SE.utf8")
        .is_err());
    let schema = RelationSchema::new("raw.orders", vec![column]).unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let input = SqlInput::inline("SELECT name FROM raw.orders");
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = [ConfiguredSqlInput::new(
        "collation",
        &input,
        "postgresql",
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let emitted: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
    assert_eq!(
        emitted["source_schemas"][0]["columns"][0]["collation"],
        "sv_SE.utf8"
    );
}

#[test]
fn direct_create_table_types_are_canonical_across_sql_dialects() {
    for dialect in ["postgresql", "duckdb", "mysql", "snowflake", "sqlite"] {
        let parser = dialect_from_name(dialect).unwrap();
        let b = analyze_inputs(
            &[SqlInput::inline(
                "CREATE TABLE t (id BIGINT, amount DECIMAL(7, 2), code VARCHAR(10))",
            )],
            dialect,
            parser.as_ref(),
        )
        .unwrap();
        let [schema] = b.source_schemas() else {
            panic!(
                "{dialect}: expected a direct DDL schema: {:?}",
                b.source_schemas()
            );
        };
        assert_eq!(schema.source_kind(), Some(SchemaSourceKind::SqlDdl));
        assert_eq!(
            schema.columns()[0].data_type(),
            &DataType::SignedInteger { bits: Some(64) }
        );
        assert_eq!(
            schema.columns()[1].data_type(),
            &DataType::Decimal {
                precision: Some(7),
                scale: Some(2)
            }
        );
        assert_eq!(
            schema.columns()[2].data_type(),
            &DataType::String {
                length: Some(10),
                fixed: false
            }
        );
        let emitted: serde_json::Value = serde_json::from_str(&to_bundle_json(&b)).unwrap();
        assert_eq!(emitted["source_schemas"][0]["source_kind"], "sql_ddl");
    }
}

#[test]
fn contradictory_direct_ddl_schemas_lose_their_physical_authority() {
    let b = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE t (id INTEGER)"),
            SqlInput::inline("CREATE TABLE t (id VARCHAR(4))"),
        ],
        "postgresql",
        &sqlparser::dialect::PostgreSqlDialect {},
    )
    .unwrap();
    assert!(b.source_schemas().is_empty());
    assert!(b
        .constraint_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "conflicting_ddl_schema"));
}

#[test]
fn catalog_overrides_conflicting_sql_schema_but_retains_proof_gap() {
    let catalog = RelationCatalog::from_schemas(&[RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("id", "TEXT", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap();
    let input = SqlInput::inline("CREATE TABLE t (id INTEGER)");
    let dialect = dialect_from_name("postgresql").unwrap();
    let cfg = [ConfiguredSqlInput::new(
        "ddl",
        &input,
        "postgresql",
        dialect.as_ref(),
    )];
    let b = analyze_configured_inputs_with_catalog(&cfg, &catalog).unwrap();
    assert_eq!(
        b.source_schemas()[0].columns()[0].data_type(),
        &DataType::String {
            length: None,
            fixed: false
        }
    );
    assert!(b
        .relation_constraints()
        .iter()
        .any(|set| set.relation() == "t"
            && set
                .diagnostics()
                .iter()
                .any(|diag| diag.code() == "ddl_catalog_schema_conflict")));
}

#[test]
fn explicit_nullable_ddl_proves_omitted_sql_null_without_overriding_real_default() {
    use sql_semantic_protocol::{ColumnValueKind, ConstraintValue};

    for dialect in ["duckdb", "postgresql", "mysql", "sqlite"] {
        let parser = dialect_from_name(dialect).unwrap();
        let bundle = analyze_inputs(
            &[SqlInput::inline(
                "CREATE TABLE t (id INTEGER, score INTEGER NULL, priority INTEGER NULL DEFAULT 7)",
            )],
            dialect,
            parser.as_ref(),
        )
        .unwrap();
        let set = bundle
            .relation_constraints()
            .iter()
            .find(|set| set.relation() == "t")
            .expect("typed DDL column sources");
        let sources = set.column_value_sources();
        let score = sources
            .iter()
            .find(|source| source.column() == "score")
            .unwrap();
        assert_eq!(score.kind(), ColumnValueKind::Default, "{dialect}");
        assert_eq!(score.value(), Some(&ConstraintValue::Null), "{dialect}");
        let priority = sources
            .iter()
            .find(|source| source.column() == "priority")
            .unwrap();
        assert_eq!(
            priority.value(),
            Some(&ConstraintValue::Integer(7)),
            "{dialect}"
        );
        assert_eq!(sources.len(), 2, "{dialect}");
    }
}

#[test]
fn portable_abs_generated_ast_is_canonical_across_every_parsing_dialect() {
    let schema = RelationSchema::new(
        "target",
        vec![
            SchemaColumn::from_sql_type("id", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("score", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let sql = SqlInput::inline(
        "CREATE TABLE target (id INTEGER, score INTEGER GENERATED ALWAYS AS (ABS(id)))",
    );
    let mut recognized = Vec::new();
    let mut parser_boundary = Vec::new();
    for name in common::DIALECTS {
        let dialect = dialect_from_name(name).unwrap();
        let input = ConfiguredSqlInput::new("generated", &sql, name, dialect.as_ref());
        match analyze_configured_inputs_with_catalog(&[input], &catalog) {
            Ok(bundle) => {
                let set = bundle
                    .relation_constraints()
                    .iter()
                    .find(|set| set.relation() == "target")
                    .expect("accepted DDL exposes canonical constraints");
                let column = set
                    .column_value_sources()
                    .iter()
                    .find(|source| source.column() == "score")
                    .expect("accepted DDL exposes generated source");
                assert_eq!(column.kind(), ColumnValueKind::Generated, "{name}");
                assert_eq!(
                    column.expression(),
                    Some(&GeneratedValueExpression::Abs(Box::new(
                        GeneratedValueExpression::Column("id".into()),
                    ))),
                    "{name}",
                );
                recognized.push(*name);
            }
            Err(_) => parser_boundary.push(name),
        }
    }
    assert!(
        recognized.contains(&"duckdb"),
        "DuckDB must recognize the positive portable fixture; rejected: {parser_boundary:?}"
    );
    assert_eq!(
        recognized.len() + parser_boundary.len(),
        common::DIALECTS.len(),
        "every exposed dialect must be classified",
    );
}
