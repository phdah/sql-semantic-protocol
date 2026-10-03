use sql_semantic_protocol::{
    analyze_configured_inputs, analyze_configured_inputs_with_catalog, ComposedSemantics,
    ConfiguredInputAnalysisError, ConfiguredSqlInput, LiteralValue, RelationCatalog,
    RelationContext, RelationResolution, RelationResolutionError, ResolvedComposedSemantics,
    SqlInput, TransformationLayer, ValueDomain,
};
use sqlparser::dialect::{dialect_from_str, PostgreSqlDialect};

fn layer_for_relation<'a>(
    layers: &'a [TransformationLayer],
    relation: &str,
) -> &'a TransformationLayer {
    layers
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|dataset| dataset.relation_name() == Some(relation))
        })
        .expect("expected named transformation layer")
}

fn resolved(layer: &TransformationLayer) -> &ResolvedComposedSemantics {
    match layer.composed_semantics() {
        ComposedSemantics::Resolved(semantics) => semantics,
        other => panic!("expected resolved composed semantics, got {other:?}"),
    }
}

#[test]
fn default_catalog_and_schema_link_inputs_and_preserve_output_bounds() {
    let dialect = PostgreSqlDialect {};
    let context =
        RelationContext::new(Some("warehouse"), Some("stage")).expect("context should be valid");
    let stage = SqlInput::inline(
        "CREATE TABLE orders AS
         SELECT id
         FROM raw.orders
         WHERE id >= 5",
    );
    let mart = SqlInput::inline(
        "CREATE TABLE final_orders AS
         SELECT id
         FROM orders
         WHERE id <= 10",
    );
    let configured = [
        ConfiguredSqlInput::new("stage", &stage, "postgresql", &dialect)
            .with_relation_context(&context),
        ConfiguredSqlInput::new("mart", &mart, "postgresql", &dialect)
            .with_relation_context(&context),
    ];

    let bundle = analyze_configured_inputs(&configured).expect("bundle should resolve");
    let layer = layer_for_relation(bundle.layers(), "warehouse.stage.final_orders");
    let semantics = resolved(layer);

    assert_eq!(
        semantics.dependencies(),
        &["warehouse.raw.orders".to_string()]
    );
    assert_eq!(
        semantics.output().columns()[0].lineage()[0].relation(),
        "warehouse.raw.orders"
    );

    let ranges = match semantics.output().columns()[0].domain() {
        ValueDomain::Ranges(ranges) => ranges.ranges(),
        other => panic!("expected bounded output domain, got {other:?}"),
    };
    assert_eq!(ranges.len(), 1);
    let lower = ranges[0].lower().expect("lower bound");
    let upper = ranges[0].upper().expect("upper bound");
    assert_eq!(
        lower.value().value(),
        &LiteralValue::Number("5".to_string())
    );
    assert!(lower.inclusive());
    assert_eq!(
        upper.value().value(),
        &LiteralValue::Number("10".to_string())
    );
    assert!(upper.inclusive());

    let edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.consumer_layer_id() == layer.id())
        .expect("consumer should have one dependency edge");
    assert_eq!(edge.relation(), "warehouse.stage.orders");
    assert_eq!(edge.resolution(), RelationResolution::Resolved);
}

#[test]
fn per_input_schema_context_distinguishes_same_named_relations() {
    let dialect = PostgreSqlDialect {};
    let sales_context =
        RelationContext::new(Some("warehouse"), Some("sales")).expect("sales context");
    let finance_context =
        RelationContext::new(Some("warehouse"), Some("finance")).expect("finance context");

    let sales = SqlInput::inline(
        "CREATE TABLE orders AS SELECT id FROM raw.sales_orders WHERE id >= 1",
    );
    let finance = SqlInput::inline(
        "CREATE TABLE orders AS SELECT id FROM raw.finance_orders WHERE id >= 100",
    );
    let report = SqlInput::inline(
        "CREATE TABLE report AS SELECT id FROM orders WHERE id <= 10",
    );
    let configured = [
        ConfiguredSqlInput::new("sales", &sales, "postgresql", &dialect)
            .with_relation_context(&sales_context),
        ConfiguredSqlInput::new("finance", &finance, "postgresql", &dialect)
            .with_relation_context(&finance_context),
        ConfiguredSqlInput::new("report", &report, "postgresql", &dialect)
            .with_relation_context(&sales_context),
    ];

    let bundle = analyze_configured_inputs(&configured).expect("contexts should disambiguate");
    let report = layer_for_relation(bundle.layers(), "warehouse.sales.report");
    let semantics = resolved(report);

    assert_eq!(
        semantics.dependencies(),
        &["warehouse.raw.sales_orders".to_string()]
    );
    let ranges = match semantics.output().columns()[0].domain() {
        ValueDomain::Ranges(ranges) => ranges.ranges(),
        other => panic!("expected report output range, got {other:?}"),
    };
    assert_eq!(
        ranges[0]
            .lower()
            .expect("sales lower bound")
            .value()
            .value(),
        &LiteralValue::Number("1".to_string())
    );
    assert_eq!(
        ranges[0]
            .upper()
            .expect("report upper bound")
            .value()
            .value(),
        &LiteralValue::Number("10".to_string())
    );
}

#[test]
fn ambiguous_partial_catalog_name_fails_explicitly() {
    let dialect = PostgreSqlDialect {};
    let input = SqlInput::inline("SELECT id FROM orders");
    let configured = [ConfiguredSqlInput::new(
        "query",
        &input,
        "postgresql",
        &dialect,
    )];
    let catalog = RelationCatalog::new(&[
        "warehouse.sales.orders",
        "warehouse.finance.orders",
    ])
    .expect("catalog should be valid");

    let error = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect_err("ambiguous relation should fail");

    match error {
        ConfiguredInputAnalysisError::RelationResolution { input_id, error } => {
            assert_eq!(input_id, "query");
            match error {
                RelationResolutionError::Ambiguous {
                    reference,
                    candidates,
                } => {
                    assert_eq!(reference, "orders");
                    assert_eq!(
                        candidates,
                        vec![
                            "warehouse.finance.orders".to_string(),
                            "warehouse.sales.orders".to_string(),
                        ]
                    );
                }
                other => panic!("expected ambiguity, got {other:?}"),
            }
        }
        other => panic!("expected relation resolution error, got {other:?}"),
    }
}

#[test]
fn quoted_and_unquoted_snowflake_identifiers_resolve_differently() {
    let dialect = dialect_from_str("snowflake").expect("snowflake dialect should exist");
    let context =
        RelationContext::new(Some("warehouse"), Some("public")).expect("context should be valid");
    let catalog = RelationCatalog::new(&[
        "WAREHOUSE.PUBLIC.ORDERS",
        "WAREHOUSE.PUBLIC.\"orders\"",
    ])
    .expect("catalog should be valid");
    let unquoted = SqlInput::inline("SELECT id FROM orders");
    let quoted = SqlInput::inline("SELECT id FROM \"orders\"");
    let configured = [
        ConfiguredSqlInput::new("unquoted", &unquoted, "snowflake", dialect.as_ref())
            .with_relation_context(&context),
        ConfiguredSqlInput::new("quoted", &quoted, "snowflake", dialect.as_ref())
            .with_relation_context(&context),
    ];

    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("quoted identities should resolve exactly");

    assert_eq!(
        bundle.layers()[0].consumes(),
        &["WAREHOUSE.PUBLIC.ORDERS".to_string()]
    );
    assert_eq!(
        bundle.layers()[1].consumes(),
        &["WAREHOUSE.PUBLIC.\"orders\"".to_string()]
    );
}

#[test]
fn missing_metadata_preserves_existing_textual_identity() {
    let dialect = PostgreSqlDialect {};
    let input = SqlInput::inline("SELECT id FROM raw.orders WHERE id > 3");
    let configured = [ConfiguredSqlInput::new(
        "query",
        &input,
        "postgresql",
        &dialect,
    )];

    let bundle = analyze_configured_inputs(&configured).expect("textual fallback should work");

    assert_eq!(bundle.layers()[0].consumes(), &["raw.orders".to_string()]);
    let semantics = resolved(&bundle.layers()[0]);
    assert_eq!(semantics.dependencies(), &["raw.orders".to_string()]);
    assert_eq!(
        semantics.output().columns()[0].lineage()[0].relation(),
        "raw.orders"
    );
}
