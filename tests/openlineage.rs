use sql_semantic_protocol::{
    analyze_inputs, to_bundle_json, to_openlineage_json, OpenLineageExportError, SqlInput,
};
use sqlparser::dialect::GenericDialect;

fn composed_chain() -> sql_semantic_protocol::AnalysisBundle {
    let dialect = GenericDialect {};
    analyze_inputs(
        &[
            SqlInput::inline(
                "CREATE TABLE stage.orders AS
                 SELECT id AS order_id, amount
                 FROM raw.orders
                 WHERE amount > 10",
            ),
            SqlInput::inline(
                "CREATE TABLE core.orders AS
                 SELECT order_id AS final_id, amount
                 FROM stage.orders",
            ),
            SqlInput::inline(
                "CREATE TABLE mart.orders AS
                 SELECT final_id, amount
                 FROM core.orders
                 WHERE amount < 100",
            ),
        ],
        "generic",
        &dialect,
    )
    .expect("representative chain should analyze")
}

#[test]
fn export_maps_composed_dataset_and_field_lineage_to_openlineage() {
    let bundle = composed_chain();
    let exported = to_openlineage_json(
        &bundle,
        "postgresql://warehouse",
        "2026-10-02T07:00:00Z",
    )
    .expect("resolved named layers should export");
    let events: serde_json::Value =
        serde_json::from_str(&exported).expect("OpenLineage export should be JSON");

    let event = events
        .as_array()
        .expect("export should be a batch array")
        .iter()
        .find(|event| event["dataset"]["name"] == "mart.orders")
        .expect("final dataset should be exported");

    assert_eq!(
        event["schemaURL"],
        "https://openlineage.io/spec/2-0-2/OpenLineage.json#/$defs/DatasetEvent"
    );
    assert_eq!(event["dataset"]["namespace"], "postgresql://warehouse");
    assert_eq!(
        event["dataset"]["facets"]["lineage"]["_schemaURL"],
        "https://openlineage.io/spec/facets/1-0-0/LineageFacet.json#/$defs/LineageDatasetFacet"
    );
    assert_eq!(
        event["dataset"]["facets"]["lineage"]["inputs"],
        serde_json::json!([
            {
                "namespace": "postgresql://warehouse",
                "name": "raw.orders",
                "type": "DATASET"
            }
        ])
    );
    assert_eq!(
        event["dataset"]["facets"]["lineage"]["fields"]["final_id"]["inputs"],
        serde_json::json!([
            {
                "namespace": "postgresql://warehouse",
                "name": "raw.orders",
                "type": "DATASET",
                "field": "id"
            }
        ])
    );
    assert_eq!(
        event["dataset"]["facets"]["lineage"]["fields"]["amount"]["inputs"],
        serde_json::json!([
            {
                "namespace": "postgresql://warehouse",
                "name": "raw.orders",
                "type": "DATASET",
                "field": "amount"
            }
        ])
    );

    let protocol: serde_json::Value = serde_json::from_str(&to_bundle_json(&bundle))
        .expect("protocol output should remain valid JSON");
    let final_layer = protocol["layers"]
        .as_array()
        .expect("layers should be an array")
        .iter()
        .find(|layer| layer["produces"][0]["name"] == "mart.orders")
        .expect("final protocol layer should remain present");

    assert_eq!(final_layer["composed_semantics"]["status"], "resolved");
    assert!(!final_layer["composed_semantics"]["column_domains"]
        .as_array()
        .expect("composed domains should be an array")
        .is_empty());
    assert!(event["dataset"]["facets"]["lineage"]
        .get("column_domains")
        .is_none());
}

#[test]
fn export_skips_anonymous_and_unresolved_layers_instead_of_inventing_dataset_identity() {
    let dialect = GenericDialect {};
    let bundle = analyze_inputs(
        &[
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.one"),
            SqlInput::inline("CREATE TABLE stage.orders AS SELECT id FROM raw.two"),
            SqlInput::inline("CREATE TABLE mart.orders AS SELECT id FROM stage.orders"),
            SqlInput::inline("SELECT id FROM raw.three"),
        ],
        "generic",
        &dialect,
    )
    .expect("ambiguous and anonymous layers should remain analyzable");

    let exported = to_openlineage_json(
        &bundle,
        "postgresql://warehouse",
        "2026-10-02T07:00:00Z",
    )
    .expect("representable layers should still export");
    let events: serde_json::Value =
        serde_json::from_str(&exported).expect("OpenLineage export should be JSON");
    let names = events
        .as_array()
        .expect("export should be an array")
        .iter()
        .map(|event| event["dataset"]["name"].as_str().expect("named dataset"))
        .collect::<Vec<_>>();

    assert!(!names.contains(&"mart.orders"));
    assert!(!names.iter().any(|name| name.starts_with("layer-")));
}

#[test]
fn export_requires_caller_owned_openlineage_identity_inputs() {
    let bundle = composed_chain();

    assert_eq!(
        to_openlineage_json(&bundle, "", "2026-10-02T07:00:00Z"),
        Err(OpenLineageExportError::EmptyNamespace)
    );
    assert_eq!(
        to_openlineage_json(&bundle, "postgresql://warehouse", ""),
        Err(OpenLineageExportError::EmptyEventTime)
    );
}
