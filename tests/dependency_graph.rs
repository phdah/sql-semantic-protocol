use sql_semantic_protocol::{
    analyze_inputs, AnalysisBundle, DatasetRef, RelationResolution, SqlInput, TransformationLayer,
};
use sqlparser::dialect::GenericDialect;

fn produced_relation(layer: &TransformationLayer) -> Option<&str> {
    layer.produces().iter().find_map(DatasetRef::relation_name)
}

fn layer_by_id<'a>(bundle: &'a AnalysisBundle, layer_id: &str) -> &'a TransformationLayer {
    bundle
        .layers()
        .iter()
        .find(|layer| layer.id() == layer_id)
        .expect("graph edge should reference an existing layer")
}

fn assert_resolved_link(
    bundle: &AnalysisBundle,
    relation: &str,
    consumer_relation: &str,
    producer_relation: &str,
) {
    let edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| {
            edge.relation() == relation
                && produced_relation(layer_by_id(bundle, edge.consumer_layer_id()))
                    == Some(consumer_relation)
        })
        .expect("expected relation dependency edge");

    assert_eq!(edge.resolution(), RelationResolution::Resolved);
    assert_eq!(edge.producer_layer_ids().len(), 1);
    assert_eq!(
        produced_relation(layer_by_id(bundle, &edge.producer_layer_ids()[0])),
        Some(producer_relation)
    );
}

#[test]
fn multi_stage_chain_links_independently_of_input_order() {
    let dialect = GenericDialect {};
    let statements = [
        "CREATE TABLE stage.orders AS SELECT id FROM raw.orders",
        "CREATE TABLE core.orders AS SELECT id FROM stage.orders",
        "CREATE TABLE mart.orders AS SELECT id FROM core.orders",
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

    for bundle in [&forward, &reverse] {
        assert_resolved_link(bundle, "stage.orders", "core.orders", "stage.orders");
        assert_resolved_link(bundle, "core.orders", "mart.orders", "core.orders");

        let external = bundle
            .graph()
            .edges()
            .iter()
            .find(|edge| edge.relation() == "raw.orders")
            .expect("raw.orders should remain explicit");
        assert_eq!(external.resolution(), RelationResolution::External);
        assert!(external.producer_layer_ids().is_empty());

        assert_eq!(bundle.graph().components().len(), 1);
        assert!(matches!(
            bundle.graph().components()[0].final_outcomes(),
            [DatasetRef::Relation { name }] if name == "mart.orders"
        ));
    }
}

#[test]
fn disconnected_chains_form_independent_components() {
    let dialect = GenericDialect {};
    let inputs = [
        SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.orders"),
        SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM stage.orders"),
        SqlInput::inline("CREATE TABLE stage.customers AS SELECT id FROM raw.customers"),
        SqlInput::inline("CREATE TABLE mart.customers AS SELECT id FROM stage.customers"),
    ];

    let bundle =
        analyze_inputs(&inputs, "generic", &dialect).expect("disconnected chains should analyze");

    assert_eq!(bundle.graph().components().len(), 2);

    let mut outcomes = bundle
        .graph()
        .components()
        .iter()
        .flat_map(|component| component.final_outcomes())
        .filter_map(DatasetRef::relation_name)
        .collect::<Vec<_>>();
    outcomes.sort();
    assert_eq!(outcomes, ["mart.customers", "mart.orders"]);

    for relation in ["raw.customers", "raw.orders"] {
        let edge = bundle
            .graph()
            .edges()
            .iter()
            .find(|edge| edge.relation() == relation)
            .expect("external dependency should be present");
        assert_eq!(edge.resolution(), RelationResolution::External);
    }
}

#[test]
fn duplicate_producers_are_explicitly_ambiguous() {
    let dialect = GenericDialect {};
    let inputs = [
        SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.orders_a"),
        SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.orders_b"),
        SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM stage.orders"),
    ];

    let bundle =
        analyze_inputs(&inputs, "generic", &dialect).expect("duplicate producers should analyze");

    let edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.relation() == "stage.orders")
        .expect("ambiguous relation should have an edge");
    assert_eq!(edge.resolution(), RelationResolution::Ambiguous);
    assert_eq!(edge.producer_layer_ids().len(), 2);
    assert!(bundle
        .graph()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "ambiguous_relation_producer"));
    assert!(bundle.graph().components()[0].final_outcomes().is_empty());
}

#[test]
fn dependency_cycles_are_detected_without_recursion() {
    let dialect = GenericDialect {};
    let inputs = [
        SqlInput::inline("CREATE TABLE model.a AS SELECT id FROM model.b"),
        SqlInput::inline("CREATE TABLE model.b AS SELECT id FROM model.a"),
    ];

    let bundle = analyze_inputs(&inputs, "generic", &dialect).expect("cycle should remain analyzable");

    assert_eq!(bundle.graph().edges().len(), 2);
    assert!(bundle
        .graph()
        .edges()
        .iter()
        .all(|edge| edge.resolution() == RelationResolution::Cycle));
    assert_eq!(bundle.graph().components().len(), 1);
    assert!(bundle.graph().components()[0].final_outcomes().is_empty());
    assert!(bundle
        .graph()
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "dependency_cycle"));
}

#[test]
fn local_cte_names_do_not_link_to_global_producers() {
    let dialect = GenericDialect {};
    let inputs = [
        SqlInput::inline("CREATE TABLE recent AS SELECT id FROM raw.one"),
        SqlInput::inline(
            "WITH recent AS (SELECT id FROM raw.two) SELECT id FROM recent",
        ),
    ];

    let bundle =
        analyze_inputs(&inputs, "generic", &dialect).expect("CTE query should analyze normally");

    assert!(!bundle
        .graph()
        .edges()
        .iter()
        .any(|edge| edge.relation() == "recent"));
    assert!(bundle
        .graph()
        .edges()
        .iter()
        .any(|edge| edge.relation() == "raw.two"
            && edge.resolution() == RelationResolution::External));
    assert_eq!(bundle.graph().components().len(), 2);
}
