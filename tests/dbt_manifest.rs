use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_dbt_manifest,
    analyze_dbt_manifest_with_schemas, parse_dbt_catalog, parse_dbt_manifest, to_bundle_json,
    ComposedSemantics, ConfiguredSqlInput, DataType, DbtArtifactsError, LiteralValue,
    RelationCatalog, RelationContext, RelationResolution, SchemaSourceKind, SqlInput,
    TransformationLayer, ValueDomain,
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

fn empty_catalog() -> sql_semantic_protocol::DbtCatalog {
    parse_dbt_catalog(
        r#"{
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
                "dbt_version": "1.11.8"
            },
            "nodes": {},
            "sources": {},
            "errors": null
        }"#,
    )
    .expect("empty dbt catalog should parse")
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
fn dbt_catalog_schema_takes_precedence_over_manifest_declared_types() {
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
    assert_eq!(source.source_kind(), Some(SchemaSourceKind::DbtCatalog));
    assert_eq!(
        source.columns()[0].data_type(),
        &DataType::SignedInteger { bits: Some(64) }
    );
    assert_eq!(
        source.columns()[1].data_type(),
        &DataType::SignedInteger { bits: Some(32) }
    );
    assert!(to_bundle_json(&bundle).contains(r#""source_kind":"dbt_catalog""#));
}

#[test]
fn dbt_catalog_schema_works_without_manifest_declared_types() {
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
            .expect("manifest fixture should be JSON");
    manifest_json["sources"]["source.demo.orders"]
        .as_object_mut()
        .expect("source should be an object")
        .remove("columns");
    let manifest =
        parse_dbt_manifest(&manifest_json.to_string()).expect("catalog-only manifest should parse");
    let catalog = fixture_catalog();
    let dialect = dialect_from_str("postgres").expect("postgres dialect");

    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "postgres", dialect.as_ref())
        .expect("catalog-only schemas should analyze");

    let source = bundle
        .source_schemas()
        .iter()
        .find(|schema| schema.relation() == "warehouse.raw.orders")
        .expect("raw source schema should be present");
    assert_eq!(source.source_kind(), Some(SchemaSourceKind::DbtCatalog));
}

#[test]
fn dbt_manifest_declared_types_fill_missing_catalog_relation() {
    let manifest = fixture_manifest();
    let catalog = empty_catalog();
    let dialect = dialect_from_str("postgres").expect("postgres dialect");

    let bundle = analyze_dbt_artifacts(&manifest, &catalog, "postgres", dialect.as_ref())
        .expect("manifest-declared source schema should be enough");

    let [source] = bundle.source_schemas() else {
        panic!("only the physical source should need fallback schema evidence");
    };
    assert_eq!(source.relation(), "warehouse.raw.orders");
    assert_eq!(source.source_kind(), Some(SchemaSourceKind::DbtManifest));
    assert_eq!(
        source.columns()[0].data_type(),
        &DataType::SignedInteger { bits: Some(64) }
    );
    assert_eq!(
        source.columns()[1].data_type(),
        &DataType::SignedInteger { bits: Some(32) }
    );
    assert_eq!(
        source.columns()[0].name(),
        "amount",
        "manifest fallback order should be deterministic"
    );
    assert_eq!(source.columns()[1].name(), "id");
    assert!(to_bundle_json(&bundle).contains(r#""source_kind":"dbt_manifest""#));
}

#[test]
fn dbt_manifest_fallback_names_columns_missing_declared_types() {
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
            .expect("manifest fixture should be JSON");
    manifest_json["sources"]["source.demo.orders"]["columns"]["amount"]
        .as_object_mut()
        .expect("amount column should be an object")
        .remove("data_type");
    let manifest =
        parse_dbt_manifest(&manifest_json.to_string()).expect("manifest should still parse");
    let catalog = empty_catalog();
    let dialect = dialect_from_str("postgres").expect("postgres dialect");

    let error = analyze_dbt_artifacts(&manifest, &catalog, "postgres", dialect.as_ref())
        .expect_err("missing required declared datatype should fail");

    assert_eq!(
        error,
        DbtArtifactsError::MissingDeclaredColumnTypes {
            relation: "warehouse.raw.orders".to_string(),
            columns: vec!["amount".to_string()],
        }
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

#[test]
fn dbt_manifest_with_schemas_matches_empty_catalog() {
    let manifest = fixture_manifest();
    let dialect = dialect_from_str(manifest.adapter_type()).expect("postgres dialect");
    let without_catalog =
        analyze_dbt_manifest_with_schemas(&manifest, manifest.adapter_type(), dialect.as_ref())
            .expect("manifest-only schemas should analyze");
    let with_empty_catalog = analyze_dbt_artifacts(
        &manifest,
        &empty_catalog(),
        manifest.adapter_type(),
        dialect.as_ref(),
    )
    .expect("empty catalog should use manifest declarations");

    assert_eq!(
        to_bundle_json(&without_catalog),
        to_bundle_json(&with_empty_catalog)
    );
    let [source] = without_catalog.source_schemas() else {
        panic!("one source schema should be emitted");
    };
    assert_eq!(source.relation(), "warehouse.raw.orders");
    assert_eq!(source.source_kind(), Some(SchemaSourceKind::DbtManifest));
    let final_orders =
        layer_for_relation(without_catalog.layers(), "warehouse.analytics.final_orders");
    let ComposedSemantics::Resolved(semantics) = final_orders.composed_semantics() else {
        panic!("final dbt model should compose");
    };
    assert_closed_number_range(semantics.output().columns()[1].domain(), "10", "50");
}

#[test]
fn dbt_manifest_without_catalog_names_each_missing_column_type() {
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
            .expect("manifest fixture should be JSON");
    for column in ["id", "amount"] {
        manifest_json["sources"]["source.demo.orders"]["columns"][column]
            .as_object_mut()
            .expect("source column")
            .remove("data_type");
    }
    let manifest = parse_dbt_manifest(&manifest_json.to_string()).expect("manifest parses");
    let dialect = dialect_from_str(manifest.adapter_type()).expect("postgres dialect");
    let error =
        analyze_dbt_manifest_with_schemas(&manifest, manifest.adapter_type(), dialect.as_ref())
            .expect_err("partial source declarations must fail");

    assert_eq!(
        error,
        DbtArtifactsError::MissingDeclaredColumnTypes {
            relation: "warehouse.raw.orders".to_string(),
            columns: vec!["amount".to_string(), "id".to_string()],
        }
    );
}

#[test]
fn dbt_manifest_without_catalog_rejects_missing_source_columns() {
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
            .expect("manifest fixture should be JSON");
    manifest_json["sources"]["source.demo.orders"]
        .as_object_mut()
        .expect("source")
        .remove("columns");
    let manifest = parse_dbt_manifest(&manifest_json.to_string()).expect("manifest parses");
    let dialect = dialect_from_str(manifest.adapter_type()).expect("postgres dialect");
    let error =
        analyze_dbt_manifest_with_schemas(&manifest, manifest.adapter_type(), dialect.as_ref())
            .expect_err("undeclared source columns must fail");

    assert_eq!(
        error,
        DbtArtifactsError::MissingCatalogSchema {
            relation: "warehouse.raw.orders".to_string(),
        }
    );
}

#[test]
fn dbt_cli_uses_manifest_schemas_without_default_catalog() {
    let manifest_path = fixture_path();
    let output = run(&[
        "--dbt-manifest",
        manifest_path.to_str().expect("UTF-8 fixture path"),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = fixture_manifest();
    let dialect = dialect_from_str(manifest.adapter_type()).expect("postgres dialect");
    let expected =
        analyze_dbt_manifest_with_schemas(&manifest, manifest.adapter_type(), dialect.as_ref())
            .expect("manifest-only library analysis");
    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("UTF-8 CLI output")
            .trim(),
        to_bundle_json(&expected)
    );
}

#[test]
fn dbt_cli_explicit_missing_catalog_is_an_error() {
    let manifest_path = fixture_path();
    let absent_catalog = manifest_path.with_file_name("missing-catalog.json");
    let output = run(&[
        "--dbt-manifest",
        manifest_path.to_str().expect("UTF-8 manifest path"),
        "--dbt-catalog",
        absent_catalog.to_str().expect("UTF-8 catalog path"),
    ]);

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing-catalog.json"),
        "error should identify the explicitly required catalog"
    );
}

#[test]
fn dbt_cli_without_catalog_names_missing_manifest_type() {
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
            .expect("manifest fixture should be JSON");
    manifest_json["sources"]["source.demo.orders"]["columns"]["amount"]
        .as_object_mut()
        .expect("source column")
        .remove("data_type");
    let path = std::env::temp_dir().join(format!(
        "sql-semantic-protocol-task49-{}-missing-type.json",
        std::process::id()
    ));
    std::fs::write(&path, manifest_json.to_string()).expect("write manifest fixture");
    let output = run(&[
        "--dbt-manifest",
        path.to_str().expect("UTF-8 temporary manifest path"),
    ]);
    std::fs::remove_file(&path).expect("remove manifest fixture");

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("warehouse.raw.orders"), "{error}");
    assert!(error.contains("amount"), "{error}");
}
