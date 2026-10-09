//! Canonical witnesses are source independent: dbt compiled SQL and direct SQL share proof facts.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, local_constructive_witnesses,
    parse_dbt_catalog, parse_dbt_manifest, ComposedSemantics, ConfiguredSqlInput, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput, WitnessDirection, WitnessOperator,
};
use sqlparser::dialect::PostgreSqlDialect;

const SQL: &str = "SELECT id, amount FROM warehouse.raw.orders WHERE amount > 10 OR id < 0";

#[test]
fn dbt_catalog_and_direct_sql_normalize_the_same_source_boolean_obligations() {
    let mut manifest: Value = serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json"))
        .expect("dbt manifest fixture");
    manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = SQL.into();
    let manifest = parse_dbt_manifest(&manifest.to_string()).expect("parsed manifest");
    let catalog =
        parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).expect("parsed catalog");
    let dialect = PostgreSqlDialect {};
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", &dialect)
        .expect("dbt compilation");
    let source = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").expect("id"),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").expect("amount"),
        ],
    )
    .expect("source schema");
    let raw_catalog = RelationCatalog::from_schemas(&[source]).expect("catalog");
    let inline = SqlInput::inline(SQL);
    let direct = analyze_configured_inputs_with_catalog(
        &[ConfiguredSqlInput::new(
            "direct",
            &inline,
            "postgresql",
            &dialect,
        )],
        &raw_catalog,
    )
    .expect("direct SQL");

    let witness = |bundle: &sql_semantic_protocol::AnalysisBundle| {
        let layer = bundle
            .layers()
            .iter()
            .find(|layer| {
                layer
                    .consumes()
                    .iter()
                    .any(|dependency| dependency == "warehouse.raw.orders")
            })
            .expect("source-consuming layer");
        let ComposedSemantics::Resolved(ref semantics) = layer.composed_semantics() else {
            panic!("resolved semantics");
        };
        local_constructive_witnesses(semantics)
            .into_iter()
            .find(|w| w.operator() == WitnessOperator::Boolean)
            .expect("boolean witness")
    };
    let dbt = witness(&dbt);
    let direct = witness(&direct);
    assert_eq!(dbt.qualifying(), direct.qualifying());
    assert_eq!(dbt.rejected(), direct.rejected());
    assert!(matches!(dbt.qualifying(), WitnessDirection::Feasible(_)));
    assert!(matches!(dbt.rejected(), WitnessDirection::Feasible(_)));
}

#[test]
fn schema_evidence_source_kinds_share_one_canonical_witness_model() {
    use sql_semantic_protocol::SchemaSourceKind;
    let dialect = PostgreSqlDialect {};
    let mut baseline = None;
    for kind in [
        SchemaSourceKind::DbtCatalog,
        SchemaSourceKind::DbtManifest,
        SchemaSourceKind::ExternalMetadata,
    ] {
        let source = RelationSchema::new(
            "warehouse.raw.orders", vec![
                SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").expect("id"),
                SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").expect("amount"),
            ]
        ).expect("source schema").with_source_kind(kind);
        let catalog = RelationCatalog::from_schemas(&[source]).expect("catalog");
        let sql_input = SqlInput::inline(SQL);
        let bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new("direct", &sql_input, "postgresql", &dialect)],
            &catalog,
        ).expect("analysis");
        let ComposedSemantics::Resolved(ref semantics) = bundle.layers()[0].composed_semantics() else {
            panic!("resolved");
        };
        let witness = local_constructive_witnesses(semantics).into_iter()
            .find(|w| w.operator() == WitnessOperator::Boolean)
            .expect("typed witness");
        if let Some(ref expected) = baseline {
            assert_eq!(witness.qualifying(), expected);
        } else {
            baseline = Some(witness.qualifying().clone());
        }
    }
}
