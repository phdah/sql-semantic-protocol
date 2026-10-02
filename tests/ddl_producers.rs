use sql_semantic_protocol::{
    analyze_inputs, to_bundle_json, DatasetRef, ProtocolStatement, SqlInput,
};
use sqlparser::dialect::{dialect_from_str, GenericDialect};
use sqlparser::parser::Parser;

const DIALECTS: &[&str] = &[
    "generic",
    "mysql",
    "postgresql",
    "postgres",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];

#[test]
fn query_backed_ddl_is_supported_across_all_exposed_dialects() {
    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");

        for (sql, expected_relation) in [
            (
                "CREATE TABLE target_table AS SELECT id FROM source_table",
                "target_table",
            ),
            (
                "CREATE VIEW target_view AS SELECT id FROM source_table",
                "target_view",
            ),
        ] {
            let bundle = analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref())
                .unwrap_or_else(|error| {
                    panic!("dialect {dialect_name} failed to analyze {sql}: {error}")
                });

            assert_eq!(
                bundle.layers().len(),
                1,
                "dialect {dialect_name} should produce one transformation layer for {sql}"
            );
            assert!(
                matches!(
                    bundle.layers()[0].produces(),
                    [DatasetRef::Relation { name }] if name == expected_relation
                ),
                "dialect {dialect_name} produced the wrong relation for {sql}"
            );

            let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
                panic!("dialect {dialect_name} should expose query semantics for {sql}");
            };
            assert_eq!(
                query.produced_relation(),
                Some(expected_relation),
                "dialect {dialect_name} should preserve the produced relation for {sql}"
            );
        }
    }
}

#[test]
fn ddl_variants_are_checked_across_all_exposed_dialects() {
    const VARIANTS: &[(&str, &str, &str)] = &[
        (
            "create or replace table",
            "CREATE OR REPLACE TABLE target_table AS SELECT id FROM source_table",
            "target_table",
        ),
        (
            "temporary table",
            "CREATE TEMPORARY TABLE target_table AS SELECT id FROM source_table",
            "target_table",
        ),
        (
            "qualified table",
            "CREATE TABLE analytics.target_table AS SELECT id FROM source_table",
            "analytics.target_table",
        ),
        (
            "quoted table",
            r#"CREATE TABLE "Target Table" AS SELECT id FROM source_table"#,
            r#""Target Table""#,
        ),
        (
            "create or replace view",
            "CREATE OR REPLACE VIEW target_view AS SELECT id FROM source_table",
            "target_view",
        ),
        (
            "materialized view",
            "CREATE MATERIALIZED VIEW target_view AS SELECT id FROM source_table",
            "target_view",
        ),
        (
            "qualified view",
            "CREATE VIEW analytics.target_view AS SELECT id FROM source_table",
            "analytics.target_view",
        ),
        (
            "quoted view",
            r#"CREATE VIEW "Target View" AS SELECT id FROM source_table"#,
            r#""Target View""#,
        ),
    ];

    for dialect_name in DIALECTS {
        let dialect =
            dialect_from_str(dialect_name).expect("documented dialect should be recognized");

        for (variant, sql, expected_relation) in VARIANTS {
            match Parser::parse_sql(dialect.as_ref(), sql) {
                Ok(_) => {
                    let bundle =
                        analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref())
                            .unwrap_or_else(|error| {
                                panic!(
                                    "dialect {dialect_name} parses {variant}, but semantic analysis failed for {sql}: {error}"
                                )
                            });

                    assert_eq!(
                        bundle.layers().len(),
                        1,
                        "dialect {dialect_name} should produce one layer for parsed {variant}"
                    );
                    assert!(
                        matches!(
                            bundle.layers()[0].produces(),
                            [DatasetRef::Relation { name }] if name == expected_relation
                        ),
                        "dialect {dialect_name} produced the wrong relation for parsed {variant}: {sql}"
                    );

                    let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0]
                    else {
                        panic!(
                            "dialect {dialect_name} should expose query semantics for parsed {variant}: {sql}"
                        );
                    };
                    assert_eq!(
                        query.produced_relation(),
                        Some(*expected_relation),
                        "dialect {dialect_name} should preserve relation identity for parsed {variant}"
                    );
                }
                Err(parse_error) => {
                    let analysis =
                        analyze_inputs(&[SqlInput::inline(sql)], dialect_name, dialect.as_ref());
                    assert!(
                        analysis.is_err(),
                        "dialect {dialect_name} parser rejects {variant}, but analysis unexpectedly accepted it: {parse_error}"
                    );
                }
            }
        }
    }
}

#[test]
fn snowflake_ctas_records_qualified_quoted_relation_and_query_semantics() {
    let dialect = dialect_from_str("snowflake").expect("snowflake dialect should exist");
    let sql = r#"CREATE OR REPLACE TEMPORARY TABLE analytics."Daily Orders" AS
                 SELECT id FROM raw.orders WHERE total_amount >= 100"#;
    let inputs = [SqlInput::inline(sql)];

    let bundle = analyze_inputs(&inputs, "snowflake", dialect.as_ref())
        .expect("snowflake CTAS should analyze");

    assert_eq!(bundle.layers().len(), 1);
    let layer = &bundle.layers()[0];
    assert_eq!(layer.input_id(), "input-0001");
    assert_eq!(layer.statement_index(), 0);
    assert_eq!(layer.consumes(), &["raw.orders".to_string()]);
    assert!(matches!(
        layer.produces(),
        [DatasetRef::Relation { name }] if name == r#"analytics."Daily Orders""#
    ));

    let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
        panic!("query-backed DDL should expose query semantics");
    };
    assert_eq!(
        query.produced_relation(),
        Some(r#"analytics."Daily Orders""#)
    );

    let plain = analyze_inputs(
        &[SqlInput::inline(
            "SELECT id FROM raw.orders WHERE total_amount >= 100",
        )],
        "snowflake",
        dialect.as_ref(),
    )
    .expect("standalone query should analyze");
    let ProtocolStatement::Query(plain_query) = &plain.inputs()[0].statements()[0] else {
        panic!("standalone SELECT should be a query");
    };

    assert_eq!(query.dependencies(), plain_query.dependencies());
    assert_eq!(query.predicates(), plain_query.predicates());
    assert_eq!(query.output(), plain_query.output());

    let json: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("protocol JSON should parse");
    assert_eq!(
        json["layers"][0]["produces"][0],
        serde_json::json!({
            "kind": "relation",
            "name": r#"analytics."Daily Orders""#
        })
    );
}

#[test]
fn postgresql_materialized_view_records_produced_relation() {
    let dialect = dialect_from_str("postgresql").expect("postgresql dialect should exist");
    let sql = r#"CREATE MATERIALIZED VIEW reporting."Order Summary" AS
                 SELECT customer_id FROM raw.orders"#;

    let bundle = analyze_inputs(&[SqlInput::inline(sql)], "postgresql", dialect.as_ref())
        .expect("postgresql materialized view should analyze");

    assert_eq!(bundle.layers().len(), 1);
    assert!(matches!(
        bundle.layers()[0].produces(),
        [DatasetRef::Relation { name }] if name == r#"reporting."Order Summary""#
    ));

    let ProtocolStatement::Query(query) = &bundle.inputs()[0].statements()[0] else {
        panic!("query-backed view should expose query semantics");
    };
    assert_eq!(
        query.produced_relation(),
        Some(r#"reporting."Order Summary""#)
    );
    assert_eq!(query.dependencies(), &["raw.orders".to_string()]);
}

#[test]
fn bare_select_produces_anonymous_layer_output() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline("SELECT id FROM raw.orders")],
        "generic",
        &dialect,
    )
    .expect("SELECT should analyze");

    assert_eq!(bundle.layers().len(), 1);
    let layer = &bundle.layers()[0];
    assert!(matches!(
        layer.produces(),
        [DatasetRef::Anonymous { layer_id }] if layer_id == layer.id()
    ));
}

#[test]
fn queryless_create_table_is_explicitly_unsupported_and_creates_no_layer() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline("CREATE TABLE standalone (id INT)")],
        "generic",
        &dialect,
    )
    .expect("queryless DDL should parse and remain explicitly unsupported");

    assert!(bundle.layers().is_empty());

    let ProtocolStatement::Unsupported(statement) = &bundle.inputs()[0].statements()[0] else {
        panic!("queryless CREATE TABLE must not be treated as a transformation");
    };
    assert_eq!(statement.category(), "create_table");
    assert_eq!(
        statement.diagnostics()[0].code(),
        "unsupported_queryless_create_table"
    );
}
