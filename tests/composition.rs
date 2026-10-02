use sql_semantic_protocol::{
    analyze_inputs, AnalysisBundle, ComposedSemantics, CompositionFailureReason, DatasetRef,
    LiteralValue, ResolvedComposedSemantics, SqlInput, TransformationLayer, ValueDomain,
};
use sqlparser::dialect::GenericDialect;

fn layer_for_relation<'a>(bundle: &'a AnalysisBundle, relation: &str) -> &'a TransformationLayer {
    bundle
        .layers()
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
fn three_stage_chain_composes_base_lineage_dependencies_and_domains() {
    let dialect = GenericDialect {};
    let inputs = [
        SqlInput::inline(
            "CREATE TABLE stage.orders AS
             SELECT id AS order_id, amount
             FROM raw.orders
             WHERE amount > 10",
        ),
        SqlInput::inline(
            "CREATE TABLE core.orders AS
             SELECT order_id AS final_id, amount
             FROM stage.orders
             WHERE amount < 100",
        ),
        SqlInput::inline(
            "CREATE TABLE mart.orders AS
             SELECT final_id, amount
             FROM core.orders
             WHERE final_id >= 5",
        ),
    ];

    let bundle = analyze_inputs(&inputs, "generic", &dialect).expect("chain should analyze");
    let semantics = resolved(layer_for_relation(&bundle, "mart.orders"));

    assert_eq!(semantics.dependencies(), &["raw.orders".to_string()]);
    assert!(semantics.diagnostics().is_empty());

    let columns = semantics.output().columns();
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].name(), "final_id");
    assert_eq!(columns[0].lineage().len(), 1);
    assert_eq!(columns[0].lineage()[0].relation(), "raw.orders");
    assert_eq!(columns[0].lineage()[0].column(), "id");
    assert_eq!(columns[1].name(), "amount");
    assert_eq!(columns[1].lineage().len(), 1);
    assert_eq!(columns[1].lineage()[0].relation(), "raw.orders");
    assert_eq!(columns[1].lineage()[0].column(), "amount");

    let amount = semantics
        .column_domains()
        .iter()
        .find(|domain| {
            domain.column().relation() == Some("raw.orders") && domain.column().name() == "amount"
        })
        .expect("amount domain should propagate to the base column");
    let amount_ranges = match amount.domain() {
        ValueDomain::Ranges(ranges) => ranges.ranges(),
        other => panic!("expected amount range, got {other:?}"),
    };
    assert_eq!(amount_ranges.len(), 1);
    assert_eq!(
        amount_ranges[0]
            .lower()
            .expect("lower amount bound")
            .value()
            .value(),
        &LiteralValue::Number("10".to_string())
    );
    assert!(!amount_ranges[0]
        .lower()
        .expect("lower amount bound")
        .inclusive());
    assert_eq!(
        amount_ranges[0]
            .upper()
            .expect("upper amount bound")
            .value()
            .value(),
        &LiteralValue::Number("100".to_string())
    );
    assert!(!amount_ranges[0]
        .upper()
        .expect("upper amount bound")
        .inclusive());

    let id = semantics
        .column_domains()
        .iter()
        .find(|domain| {
            domain.column().relation() == Some("raw.orders") && domain.column().name() == "id"
        })
        .expect("renamed id domain should propagate to the base column");
    let id_ranges = match id.domain() {
        ValueDomain::Ranges(ranges) => ranges.ranges(),
        other => panic!("expected id range, got {other:?}"),
    };
    assert_eq!(
        id_ranges[0]
            .lower()
            .expect("id lower bound")
            .value()
            .value(),
        &LiteralValue::Number("5".to_string())
    );
    assert!(id_ranges[0].lower().expect("id lower bound").inclusive());
}

#[test]
fn composition_is_independent_of_input_order() {
    let dialect = GenericDialect {};
    let statements = [
        "CREATE TABLE stage.orders AS SELECT id AS order_id FROM raw.orders",
        "CREATE TABLE core.orders AS SELECT order_id AS final_id FROM stage.orders",
        "CREATE TABLE mart.orders AS SELECT final_id FROM core.orders WHERE final_id >= 5",
    ];

    let forward = analyze_inputs(
        &statements
            .iter()
            .map(|sql| SqlInput::inline(*sql))
            .collect::<Vec<_>>(),
        "generic",
        &dialect,
    )
    .expect("forward chain should analyze");
    let reverse = analyze_inputs(
        &statements
            .iter()
            .rev()
            .map(|sql| SqlInput::inline(*sql))
            .collect::<Vec<_>>(),
        "generic",
        &dialect,
    )
    .expect("reverse chain should analyze");

    assert_eq!(
        resolved(layer_for_relation(&forward, "mart.orders")),
        resolved(layer_for_relation(&reverse, "mart.orders"))
    );
}

#[test]
fn computed_columns_keep_lineage_but_stop_precise_domain_propagation() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT amount + 1 AS adjusted
                 FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.orders AS
                 SELECT adjusted
                 FROM stage.orders
                 WHERE adjusted > 10",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("computed chain should analyze");

    let semantics = resolved(layer_for_relation(&bundle, "mart.orders"));
    assert!(semantics.column_domains().is_empty());
    assert!(semantics
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "non_invertible_column_transform"));

    let lineage = semantics.output().columns()[0].lineage();
    assert_eq!(lineage.len(), 1);
    assert_eq!(lineage[0].relation(), "raw.orders");
    assert_eq!(lineage[0].column(), "amount");
}

#[test]
fn join_outputs_preserve_multiple_transitive_sources() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT customer_id, amount
                 FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE stage.customers AS
                 SELECT id, score
                 FROM raw.customers",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.summary AS
                 SELECT o.amount + c.score AS combined
                 FROM stage.orders AS o
                 JOIN stage.customers AS c ON o.customer_id = c.id",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("join chain should analyze");

    let semantics = resolved(layer_for_relation(&bundle, "mart.summary"));
    assert_eq!(
        semantics.dependencies(),
        &["raw.customers".to_string(), "raw.orders".to_string()]
    );

    let lineage = semantics.output().columns()[0].lineage();
    assert_eq!(lineage.len(), 2);
    assert_eq!(lineage[0].relation(), "raw.customers");
    assert_eq!(lineage[0].column(), "score");
    assert_eq!(lineage[1].relation(), "raw.orders");
    assert_eq!(lineage[1].column(), "amount");
}

#[test]
fn disconnected_components_compose_independently() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.orders"),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM stage.orders"),
            SqlInput::inline("CREATE TABLE stage.customers AS SELECT id FROM raw.customers"),
            SqlInput::inline("CREATE TABLE mart.customers AS SELECT id FROM stage.customers"),
        ],
        "generic",
        &dialect,
    )
    .expect("disconnected chains should analyze");

    assert_eq!(
        resolved(layer_for_relation(&bundle, "mart.orders")).dependencies(),
        &["raw.orders".to_string()]
    );
    assert_eq!(
        resolved(layer_for_relation(&bundle, "mart.customers")).dependencies(),
        &["raw.customers".to_string()]
    );
}

#[test]
fn ambiguous_producers_leave_composition_explicitly_unresolved() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.one"),
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.two"),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM stage.orders"),
        ],
        "generic",
        &dialect,
    )
    .expect("ambiguous graph should remain analyzable");

    match layer_for_relation(&bundle, "mart.orders").composed_semantics() {
        ComposedSemantics::Unresolved(semantics) => {
            assert_eq!(
                semantics.reason(),
                CompositionFailureReason::AmbiguousProducer
            );
            assert!(semantics
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code() == "ambiguous_relation_producer"));
        }
        other => panic!("expected unresolved composition, got {other:?}"),
    }
}

#[test]
fn bare_query_remains_a_resolved_anonymous_outcome() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline("SELECT id FROM raw.orders")],
        "generic",
        &dialect,
    )
    .expect("bare query should analyze");

    assert!(matches!(
        bundle.layers()[0].produces(),
        [DatasetRef::Anonymous { .. }]
    ));
    assert!(matches!(
        bundle.layers()[0].composed_semantics(),
        ComposedSemantics::Resolved(_)
    ));
}
