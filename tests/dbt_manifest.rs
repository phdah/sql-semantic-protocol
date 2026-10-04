use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_dbt_manifest,
    parse_dbt_catalog, parse_dbt_manifest, to_bundle_json, ComposedSemantics, ConfiguredSqlInput,
    LiteralValue, RelationCatalog,
    RelationContext, RelationResolution, SqlInput, TransformationLayer, ValueDomain,
};
use sqlparser::dialect::dialect_from_str;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dbt/manifest-v12.json")
}

fn fixture_manifest() -> sql_semantic_protocol::DbtManifest {
    parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("dbt manifest fixture should parse")
}

fn fixture_catalog() -> sql_semantic_protocol::DbtCatalog {
    parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json"))
        .expect("dbt catalog fixture should parse")
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sql-semantic-protocol"))
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("CLI should run")
}

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

fn assert_closed_number_range(domain: &ValueDomain, lower: &str, upper: &str) {
    let ValueDomain::Ranges(ranges) = domain else {
        panic!("expected ranges domain, got {domain:?}");
    };
    let [range] = ranges.ranges() else {
        panic!("expected one output range");
    };
    let lower_bound = range.lower().expect("lower bound");
    let upper_bound = range.upper().expect("upper bound");

    assert_eq!(
        lower_bound.value().value(),
        &LiteralValue::Number(lower.to_string())
    );
    assert!(lower_bound.inclusive());
    assert_eq!(
        upper_bound.value().value(),
        &LiteralValue::Number(upper.to_string())
    );
    assert!(upper_bound.inclusive());
}

#[test]
fn dbt_manifest_builds_named_graph_and_composed_outcome_domains() {
    let manifest = fixture_manifest();
    assert_eq!(manifest.schema_version(), 12);
    assert_eq!(manifest.dbt_version(), Some("1.11.8"));
    assert_eq!(manifest.adapter_type(), "postgres");

    let dialect = dialect_from_str(manifest.adapter_type()).expect("postgres dialect");
    let bundle = analyze_dbt_manifest(&manifest, manifest.adapter_type(), dialect.as_ref())
        .expect("dbt manifest should analyze");

    assert_eq!(bundle.layers().len(), 2);
    assert_eq!(bundle.layers()[0].input_id(), "model.demo.stg_orders");
    assert_eq!(bundle.layers()[1].input_id(), "model.demo.final_orders");

    let stage = layer_for_relation(bundle.layers(), "warehouse.analytics.stg_orders");
    let stage_edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.consumer_layer_id() == stage.id())
        .expect("stage should consume the dbt source");
    assert_eq!(stage_edge.relation(), "warehouse.raw.orders");
    assert_eq!(stage_edge.resolution(), RelationResolution::External);

    let final_orders = layer_for_relation(bundle.layers(), "warehouse.analytics.final_orders");
    let final_edge = bundle
        .graph()
        .edges()
        .iter()
        .find(|edge| edge.consumer_layer_id() == final_orders.id())
        .expect("final model should consume the stage model");
    assert_eq!(final_edge.relation(), "warehouse.analytics.stg_orders");
    assert_eq!(final_edge.resolution(), RelationResolution::Resolved);

    let ComposedSemantics::Resolved(semantics) = final_orders.composed_semantics() else {
        panic!("final dbt model should compose");
    };
    assert_eq!(
        semantics.dependencies(),
        &["warehouse.raw.orders".to_string()]
    );
    assert_eq!(
        semantics.output().columns()[1].lineage()[0].relation(),
        "warehouse.raw.orders"
    );
    assert_closed_number_range(semantics.output().columns()[1].domain(), "10", "50");
}

#[test]
fn dbt_manifest_and_catalog_emit_complete_typed_relation_schemas() {
    let manifest = fixture_manifest();
    let catalog = fixture_catalog();
    let dialect = dialect_from_str("postgres").expect("postgres dialect");
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "postgres", dialect.as_ref())
        .expect("paired dbt artifacts should analyze");

    assert_eq!(bundle.source_schemas().len(), 3);
    let source = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "warehouse.raw.orders")
        .expect("raw source schema should be present");
    assert_eq!(
        source
            .columns()
            .iter()
            .map(|column| (column.name(), column.data_type().kind()))
            .collect::<Vec<_>>(),
        [("id", "signed_integer"), ("amount", "signed_integer")]
    );
}

#[test]
fn dbt_adapter_matches_equivalent_generic_analysis() {
    let manifest = fixture_manifest();
    let dialect = dialect_from_str("postgres").expect("postgres dialect");
    let adapted = analyze_dbt_manifest(&manifest, "postgres", dialect.as_ref())
        .expect("dbt manifest should analyze");

    let context =
        RelationContext::new(Some("warehouse"), Some("analytics")).expect("relation context");
    let inputs = [
        SqlInput::file(
            "models/stg_orders.sql",
            "CREATE VIEW warehouse.analytics.stg_orders AS\nSELECT id, amount FROM warehouse.raw.orders WHERE amount >= 10 AND amount <= 100",
        ),
        SqlInput::file(
            "models/final_orders.sql",
            "CREATE VIEW warehouse.analytics.final_orders AS\nSELECT id, amount FROM warehouse.analytics.stg_orders WHERE amount <= 50",
        ),
    ];
    let configured = [
        ConfiguredSqlInput::new(
            "model.demo.stg_orders",
            &inputs[0],
            "postgres",
            dialect.as_ref(),
        )
        .with_relation_context(&context),
        ConfiguredSqlInput::new(
            "model.demo.final_orders",
            &inputs[1],
            "postgres",
            dialect.as_ref(),
        )
        .with_relation_context(&context),
    ];
    let catalog = RelationCatalog::new(&[
        "warehouse.analytics.final_orders",
        "warehouse.analytics.stg_orders",
        "warehouse.raw.orders",
    ])
    .expect("catalog should be valid");
    let generic = analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("equivalent generic inputs should analyze");

    assert_eq!(to_bundle_json(&adapted), to_bundle_json(&generic));
}

#[test]
fn repeated_dbt_analysis_is_byte_identical() {
    let manifest = fixture_manifest();
    let dialect = dialect_from_str("postgres").expect("postgres dialect");

    let first =
        analyze_dbt_manifest(&manifest, "postgres", dialect.as_ref()).expect("first analysis");
    let second =
        analyze_dbt_manifest(&manifest, "postgres", dialect.as_ref()).expect("second analysis");

    assert_eq!(to_bundle_json(&first), to_bundle_json(&second));
}

#[test]
fn dbt_manifest_cli_matches_library_analysis() {
    let manifest_path = fixture_path();
    let catalog_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dbt/catalog-v1.json");
    let output = run(&[
        "--dbt-manifest",
        manifest_path.to_str().expect("UTF-8 fixture path"),
        "--dbt-catalog",
        catalog_path.to_str().expect("UTF-8 catalog fixture path"),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let manifest = fixture_manifest();
    let catalog = fixture_catalog();
    let dialect = dialect_from_str("postgres").expect("postgres dialect");
    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "postgres", dialect.as_ref())
        .expect("library analysis");

    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("stdout should be UTF-8")
            .trim(),
        to_bundle_json(&bundle)
    );
}
