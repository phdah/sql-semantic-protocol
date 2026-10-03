use sql_semantic_protocol::{
    analyze_inputs, select_targets, ComposedSemantics, SqlInput, TargetSelectionError,
};
use sqlparser::dialect::GenericDialect;

fn analyze_fixture() -> sql_semantic_protocol::AnalysisBundle {
    let dialect = GenericDialect {};
    analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id, amount FROM raw.orders"),
            SqlInput::inline(
                "CREATE TABLE core.orders AS SELECT id, amount FROM stage.orders WHERE amount > 10",
            ),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id, amount FROM core.orders"),
            SqlInput::inline("CREATE TABLE mart.audit AS SELECT id FROM raw.audit"),
            SqlInput::inline("CREATE TABLE mart.customers AS SELECT id FROM raw.customers"),
        ],
        "generic",
        &dialect,
    )
    .expect("target-selection fixture should analyze")
}

fn produced_relations(bundle: &sql_semantic_protocol::AnalysisBundle) -> Vec<&str> {
    bundle
        .layers()
        .iter()
        .flat_map(|layer| layer.produces())
        .filter_map(|dataset| dataset.relation_name())
        .collect()
}

#[test]
fn no_targets_preserve_the_complete_bundle() {
    let bundle = analyze_fixture();
    let selected = select_targets(&bundle, &[]).expect("empty target selection should succeed");

    assert_eq!(selected, bundle);
}

#[test]
fn deep_target_keeps_required_ancestors_and_omits_unrelated_components() {
    let bundle = analyze_fixture();
    let selected = select_targets(&bundle, &["mart.orders".to_string()])
        .expect("known target should be selected");

    assert_eq!(
        produced_relations(&selected),
        vec!["stage.orders", "core.orders", "mart.orders"]
    );
    assert_eq!(selected.inputs().len(), bundle.inputs().len());
    assert_eq!(selected.graph().components().len(), 1);
    assert_eq!(
        selected.graph().components()[0].final_outcomes()[0].relation_name(),
        Some("mart.orders")
    );

    let target_layer = selected
        .layers()
        .iter()
        .find(|layer| {
            layer
                .produces()
                .iter()
                .any(|dataset| dataset.relation_name() == Some("mart.orders"))
        })
        .expect("selected target layer should be present");

    let ComposedSemantics::Resolved(semantics) = target_layer.composed_semantics() else {
        panic!("target semantics should remain resolved");
    };
    assert_eq!(semantics.dependencies(), &["raw.orders".to_string()]);
    assert_eq!(
        semantics.output().columns()[0].lineage()[0].relation(),
        "raw.orders"
    );
}

#[test]
fn multiple_unrelated_targets_are_selected_together() {
    let bundle = analyze_fixture();
    let selected = select_targets(
        &bundle,
        &["mart.orders".to_string(), "mart.audit".to_string()],
    )
    .expect("multiple known targets should be selected");

    assert_eq!(
        produced_relations(&selected),
        vec!["stage.orders", "core.orders", "mart.orders", "mart.audit"]
    );
    assert_eq!(selected.graph().components().len(), 2);
    assert!(!produced_relations(&selected).contains(&"mart.customers"));
}

#[test]
fn unknown_target_is_an_explicit_error() {
    let bundle = analyze_fixture();
    let error = select_targets(&bundle, &["orders".to_string()])
        .expect_err("unqualified target should not match a qualified producer");

    assert_eq!(error.target(), "orders");
    assert!(matches!(&error, TargetSelectionError::UnknownTarget { .. }));
    assert_eq!(
        error.to_string(),
        "target relation 'orders' is not produced by any supplied transformation"
    );
}

#[test]
fn ambiguous_target_is_an_explicit_error() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM raw.orders"),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM archive.orders"),
        ],
        "generic",
        &dialect,
    )
    .expect("ambiguous-target fixture should analyze");

    let error = select_targets(&bundle, &["mart.orders".to_string()])
        .expect_err("duplicate producers should make the target ambiguous");

    assert_eq!(error.target(), "mart.orders");
    assert_eq!(
        error.producer_layer_ids(),
        &["layer-0001".to_string(), "layer-0002".to_string()]
    );
    assert!(matches!(
        error,
        TargetSelectionError::AmbiguousTarget { .. }
    ));
}
