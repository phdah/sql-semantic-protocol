use sql_semantic_protocol::{analyze_inputs, to_bundle_json, AnalysisBundle, SqlInput};
use sqlparser::dialect::GenericDialect;

fn analyze_fixture() -> AnalysisBundle {
    let dialect = GenericDialect {};
    analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id, amount FROM raw.orders"),
            SqlInput::inline(
                "CREATE TABLE mart.orders AS
                 SELECT id, amount FROM stage.orders WHERE amount > 10",
            ),
            SqlInput::inline("SELECT id FROM raw.audit"),
            SqlInput::inline("CREATE TABLE stage.customers AS SELECT id FROM raw.customers"),
            SqlInput::inline("CREATE TABLE mart.customers AS SELECT id FROM stage.customers"),
        ],
        "generic",
        &dialect,
    )
    .expect("outcome fixture should analyze")
}

fn relation_name(layer: &serde_json::Value) -> Option<&str> {
    layer["produces"]
        .as_array()?
        .iter()
        .find_map(|dataset| dataset["name"].as_str())
}

#[test]
fn one_protocol_contains_all_outcomes_and_terminal_classification() {
    let bundle = analyze_fixture();
    let output: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("protocol output should be JSON");

    let layers = output["layers"]
        .as_array()
        .expect("layers should be an array");
    assert_eq!(layers.len(), 5);
    assert!(layers
        .iter()
        .any(|layer| relation_name(layer) == Some("stage.orders")));
    assert!(layers
        .iter()
        .any(|layer| relation_name(layer) == Some("mart.orders")));
    assert!(layers
        .iter()
        .any(|layer| relation_name(layer) == Some("stage.customers")));
    assert!(layers
        .iter()
        .any(|layer| relation_name(layer) == Some("mart.customers")));

    let terminal_outcomes = output["graph"]["components"]
        .as_array()
        .expect("components should be an array")
        .iter()
        .flat_map(|component| {
            component["final_outcomes"]
                .as_array()
                .expect("final outcomes should be an array")
                .iter()
        })
        .collect::<Vec<_>>();

    assert_eq!(terminal_outcomes.len(), 3);
    assert_eq!(
        terminal_outcomes
            .iter()
            .filter_map(|dataset| dataset["name"].as_str())
            .collect::<Vec<_>>(),
        vec!["mart.orders", "mart.customers"]
    );
    assert!(terminal_outcomes.iter().any(|dataset| {
        dataset["kind"] == "anonymous" && dataset["layer_id"] == "layer-0003"
    }));

    for terminal_outcome in &terminal_outcomes {
        assert!(layers.iter().any(|layer| {
            layer["produces"]
                .as_array()
                .expect("produces should be an array")
                .iter()
                .any(|produced| produced == *terminal_outcome)
        }));
    }

    let mart_orders = layers
        .iter()
        .find(|layer| relation_name(layer) == Some("mart.orders"))
        .expect("mart.orders should be present");
    assert_eq!(
        mart_orders["composed_semantics"]["dependencies"],
        serde_json::json!(["raw.orders"])
    );
    assert_eq!(
        mart_orders["composed_semantics"]["output"]["columns"][0]["lineage"][0]["relation"],
        "raw.orders"
    );
}

#[test]
fn consumer_can_select_terminal_layers_from_the_complete_document() {
    let bundle = analyze_fixture();
    let output: serde_json::Value =
        serde_json::from_str(&to_bundle_json(&bundle)).expect("protocol output should be JSON");

    let layers = output["layers"]
        .as_array()
        .expect("layers should be an array");
    let terminal_outcomes = output["graph"]["components"]
        .as_array()
        .expect("components should be an array")
        .iter()
        .flat_map(|component| {
            component["final_outcomes"]
                .as_array()
                .expect("final outcomes should be an array")
                .iter()
        })
        .collect::<Vec<_>>();

    let terminal_layers = layers
        .iter()
        .filter(|layer| {
            layer["produces"]
                .as_array()
                .expect("produces should be an array")
                .iter()
                .any(|produced| terminal_outcomes.contains(&produced))
        })
        .collect::<Vec<_>>();

    assert_eq!(layers.len(), 5);
    assert_eq!(terminal_layers.len(), 3);
}
