use sql_semantic_protocol::{
    analyze_inputs, to_bundle_json, to_bundle_json_with_scope, AnalysisBundle, OutputScope, SqlInput,
};
use sqlparser::dialect::GenericDialect;

fn analyze_fixture() -> AnalysisBundle {
    let dialect = GenericDialect {};
    analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS SELECT id, amount FROM raw.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.orders AS
                 SELECT id, amount FROM stage.orders WHERE amount > 10",
            ),
            SqlInput::inline("SELECT id FROM raw.audit"),
            SqlInput::inline(
                "CREATE TABLE stage.customers AS SELECT id FROM raw.customers",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.customers AS SELECT id FROM stage.customers",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("scope fixture should analyze")
}

fn json_for(bundle: &AnalysisBundle, scope: OutputScope) -> serde_json::Value {
    serde_json::from_str(&to_bundle_json_with_scope(bundle, scope))
        .expect("scoped protocol output should be JSON")
}

fn relation_name(layer: &serde_json::Value) -> Option<&str> {
    layer["produces"]
        .as_array()?
        .iter()
        .find_map(|dataset| dataset["name"].as_str())
}

#[test]
fn final_scope_exposes_terminal_outcomes_across_related_and_unrelated_components() {
    let bundle = analyze_fixture();
    let final_output = json_for(&bundle, OutputScope::Final);
    let all_output = json_for(&bundle, OutputScope::AllLayers);

    assert_eq!(final_output["inputs"], all_output["inputs"]);
    assert_eq!(final_output["graph"], all_output["graph"]);

    let final_layers = final_output["layers"]
        .as_array()
        .expect("final layers should be an array");
    let all_layers = all_output["layers"]
        .as_array()
        .expect("all layers should be an array");

    assert_eq!(final_layers.len(), 3);
    assert_eq!(all_layers.len(), 5);

    let final_named = final_layers
        .iter()
        .filter_map(relation_name)
        .collect::<Vec<_>>();
    assert_eq!(final_named, vec!["mart.orders", "mart.customers"]);
    assert!(final_layers
        .iter()
        .any(|layer| layer["produces"][0]["kind"] == "anonymous"));
    assert!(!final_layers
        .iter()
        .any(|layer| relation_name(layer) == Some("stage.orders")));
    assert!(!final_layers
        .iter()
        .any(|layer| relation_name(layer) == Some("stage.customers")));

    let mart_orders = final_layers
        .iter()
        .find(|layer| relation_name(layer) == Some("mart.orders"))
        .expect("mart.orders should be a final outcome");
    assert_eq!(
        mart_orders["composed_semantics"]["dependencies"],
        serde_json::json!(["raw.orders"])
    );
    assert_eq!(
        mart_orders["composed_semantics"]["output"]["columns"][0]["lineage"][0]["relation"],
        "raw.orders"
    );

    for final_layer in final_layers {
        let layer_id = final_layer["id"]
            .as_str()
            .expect("final layer should have an id");
        let all_layer = all_layers
            .iter()
            .find(|layer| layer["id"] == layer_id)
            .expect("final layer should also exist in all-layer output");
        assert_eq!(final_layer, all_layer);
    }
}

#[test]
fn all_layer_scope_keeps_every_transformation_and_graph_relationship() {
    let bundle = analyze_fixture();
    let output = json_for(&bundle, OutputScope::AllLayers);

    assert_eq!(output["layers"].as_array().map(Vec::len), Some(5));
    assert!(output["graph"]["edges"]
        .as_array()
        .expect("graph edges should be an array")
        .iter()
        .any(|edge| {
            edge["relation"] == "stage.orders"
                && edge["resolution"] == "resolved"
                && edge["producer_layer_ids"]
                    .as_array()
                    .is_some_and(|ids| ids.len() == 1)
        }));
}

#[test]
fn one_layer_bundle_is_identical_under_both_scopes() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[SqlInput::inline("SELECT id FROM raw.orders")],
        "generic",
        &dialect,
    )
    .expect("single query should analyze");

    assert_eq!(
        to_bundle_json_with_scope(&bundle, OutputScope::Final),
        to_bundle_json_with_scope(&bundle, OutputScope::AllLayers)
    );
    assert_eq!(
        to_bundle_json(&bundle),
        to_bundle_json_with_scope(&bundle, OutputScope::AllLayers)
    );
}
