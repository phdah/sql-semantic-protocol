use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, select_targets, to_bundle_json, ComposedSemantics,
    CompositionFailureReason, ConfiguredSqlInput, LiteralValue, RelationCatalog, RelationContext,
    SqlInput, TransformationLayer, ValueDomain,
};
use sqlparser::dialect::{GenericDialect, MySqlDialect, PostgreSqlDialect, SnowflakeDialect};

const CATALOG_RELATIONS: &[&str] = &[
    "warehouse.analytics.audit_report",
    "warehouse.analytics.final_orders",
    "warehouse.analytics.order_audit",
    "warehouse.analytics.stage_orders",
    "warehouse.finance.snapshot",
    "warehouse.raw.customers",
    "warehouse.raw.orders",
];

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/extended_bundle")
        .join(name)
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

fn configured_fixture_bundle() -> sql_semantic_protocol::AnalysisBundle {
    let generic = GenericDialect {};
    let postgres = PostgreSqlDialect {};
    let snowflake = SnowflakeDialect {};
    let mysql = MySqlDialect {};
    let analytics_context =
        RelationContext::new(Some("warehouse"), Some("analytics")).expect("analytics context");
    let finance_context =
        RelationContext::new(Some("warehouse"), Some("finance")).expect("finance context");
    let catalog = RelationCatalog::new(CATALOG_RELATIONS).expect("catalog should be valid");

    let inputs = [
        SqlInput::file(
            "sql/stage_orders.sql",
            include_str!("fixtures/extended_bundle/sql/stage_orders.sql"),
        ),
        SqlInput::file(
            "sql/append_order_audit.sql",
            include_str!("fixtures/extended_bundle/sql/append_order_audit.sql"),
        ),
        SqlInput::file(
            "sql/audit_report.sql",
            include_str!("fixtures/extended_bundle/sql/audit_report.sql"),
        ),
        SqlInput::file(
            "sql/final_orders.sql",
            include_str!("fixtures/extended_bundle/sql/final_orders.sql"),
        ),
        SqlInput::file(
            "sql/finance_snapshot.sql",
            include_str!("fixtures/extended_bundle/sql/finance_snapshot.sql"),
        ),
    ];
    let configured = [
        ConfiguredSqlInput::new("stage-orders", &inputs[0], "postgresql", &postgres)
            .with_relation_context(&analytics_context),
        ConfiguredSqlInput::new("append-order-audit", &inputs[1], "generic", &generic)
            .with_relation_context(&analytics_context),
        ConfiguredSqlInput::new("audit-report", &inputs[2], "postgresql", &postgres)
            .with_relation_context(&analytics_context),
        ConfiguredSqlInput::new("final-orders", &inputs[3], "snowflake", &snowflake)
            .with_relation_context(&analytics_context),
        ConfiguredSqlInput::new("finance-snapshot", &inputs[4], "mysql", &mysql)
            .with_relation_context(&finance_context),
    ];

    analyze_configured_inputs_with_catalog(&configured, &catalog)
        .expect("extended configured bundle should analyze")
}

#[test]
fn extended_manifest_matches_api_and_preserves_outcome_semantics() {
    let manifest = fixture_path("analysis-all.json");
    let output = run(&["--manifest", manifest.to_str().expect("UTF-8 fixture path")]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());

    let bundle = configured_fixture_bundle();
    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("stdout should be UTF-8")
            .trim(),
        to_bundle_json(&bundle)
    );

    assert_eq!(bundle.graph().components().len(), 2);

    let final_orders = layer_for_relation(bundle.layers(), "warehouse.analytics.final_orders");
    let ComposedSemantics::Resolved(final_semantics) = final_orders.composed_semantics() else {
        panic!("final orders should compose");
    };
    assert_eq!(
        final_semantics.dependencies(),
        &["warehouse.raw.orders".to_string()]
    );
    assert_eq!(
        final_semantics.output().columns()[1].lineage()[0].relation(),
        "warehouse.raw.orders"
    );
    assert_closed_number_range(final_semantics.output().columns()[1].domain(), "5", "10");

    let append = layer_for_relation(bundle.layers(), "warehouse.analytics.order_audit");
    let ComposedSemantics::Resolved(append_semantics) = append.composed_semantics() else {
        panic!("INSERT-select rows should compose through stage_orders");
    };
    assert_closed_number_range(append_semantics.output().columns()[1].domain(), "5", "20");

    let audit_report = layer_for_relation(bundle.layers(), "warehouse.analytics.audit_report");
    let ComposedSemantics::Unresolved(audit_semantics) = audit_report.composed_semantics() else {
        panic!("downstream reader of an append-only producer must remain partial");
    };
    assert_eq!(
        audit_semantics.reason(),
        CompositionFailureReason::PartialProducer
    );
    assert!(audit_semantics
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code() == "partial_relation_producer"));

    let finance = layer_for_relation(bundle.layers(), "warehouse.finance.snapshot");
    let ComposedSemantics::Resolved(finance_semantics) = finance.composed_semantics() else {
        panic!("independent finance pipeline should compose");
    };
    assert_eq!(
        finance_semantics.dependencies(),
        &["warehouse.raw.customers".to_string()]
    );
}

#[test]
fn target_manifest_matches_post_analysis_target_projection() {
    let manifest = fixture_path("analysis-target.json");
    let output = run(&["--manifest", manifest.to_str().expect("UTF-8 fixture path")]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bundle = configured_fixture_bundle();
    let target = "warehouse.analytics.final_orders".to_string();
    let selected = select_targets(&bundle, &[target]).expect("target should resolve");

    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("stdout should be UTF-8")
            .trim(),
        to_bundle_json(&selected)
    );
    assert_eq!(selected.layers().len(), 2);
    assert!(selected.layers().iter().any(
        |layer| layer.produces()[0].relation_name() == Some("warehouse.analytics.stage_orders")
    ));
    assert!(selected.layers().iter().any(
        |layer| layer.produces()[0].relation_name() == Some("warehouse.analytics.final_orders")
    ));
}

#[test]
fn repeated_manifest_analysis_is_byte_identical() {
    let manifest = fixture_path("analysis-all.json");
    let path = manifest.to_str().expect("UTF-8 fixture path");
    let first = run(&["--manifest", path]);
    let second = run(&["--manifest", path]);

    assert!(first.status.success());
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stderr, second.stderr);
}

#[test]
fn direct_cli_catalog_metadata_matches_equivalent_manifest() {
    let root = std::env::temp_dir().join(format!(
        "sql-semantic-protocol-extended-cli-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary fixture directory");

    let manifest_path = root.join("analysis.json");
    let manifest = serde_json::json!({
        "manifest_version": "1",
        "dialect": "generic",
        "catalog_relations": [
            "warehouse.raw.orders",
            "warehouse.analytics.orders",
            "warehouse.analytics.final_orders"
        ],
        "relation_context": {
            "default_catalog": "warehouse",
            "default_schema": "analytics"
        },
        "output_scope": "targets",
        "targets": ["warehouse.analytics.final_orders"],
        "inputs": [
            {
                "id": "input-0001",
                "sql": "CREATE TABLE orders AS SELECT id FROM raw.orders WHERE id >= 5"
            },
            {
                "id": "input-0002",
                "sql": "CREATE TABLE final_orders AS SELECT id FROM orders WHERE id <= 10"
            }
        ]
    });
    fs::write(&manifest_path, manifest.to_string()).expect("manifest should be written");

    let manifest_output = run(&[
        "--manifest",
        manifest_path.to_str().expect("UTF-8 manifest path"),
    ]);
    let direct_output = run(&[
        "--dialect",
        "generic",
        "--default-catalog",
        "warehouse",
        "--default-schema",
        "analytics",
        "--catalog-relation",
        "warehouse.raw.orders",
        "--catalog-relation",
        "warehouse.analytics.orders",
        "--catalog-relation",
        "warehouse.analytics.final_orders",
        "--target",
        "warehouse.analytics.final_orders",
        "--sql",
        "CREATE TABLE orders AS SELECT id FROM raw.orders WHERE id >= 5",
        "--sql",
        "CREATE TABLE final_orders AS SELECT id FROM orders WHERE id <= 10",
    ]);

    fs::remove_dir_all(&root).expect("temporary fixture should be removed");

    assert!(
        manifest_output.status.success(),
        "{}",
        String::from_utf8_lossy(&manifest_output.stderr)
    );
    assert!(
        direct_output.status.success(),
        "{}",
        String::from_utf8_lossy(&direct_output.stderr)
    );
    assert_eq!(direct_output.stdout, manifest_output.stdout);
    assert_eq!(direct_output.stderr, manifest_output.stderr);
}
