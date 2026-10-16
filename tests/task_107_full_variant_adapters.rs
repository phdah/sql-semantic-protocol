//! TASK-107 direct SQL <-> compiled dbt per-variant adapter differential.
//!
//! Replays *each* frozen mandatory expression variant through the same
//! warehouse-typed physical source schema, from two independent input paths.
//! Equality is required for output domains, input bounds, exactness and both
//! Boolean witness directions. This test does not claim an opaque UDF is exact.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, ComparisonAssumption, ComposedSemantics,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const FREEZE: &str = include_str!("../docs/task-107-acceptance-freeze.json");

fn source_semantics(
    bundle: &sql_semantic_protocol::AnalysisBundle,
) -> &sql_semantic_protocol::ResolvedComposedSemantics {
    let source = bundle
        .layers()
        .iter()
        .find(|layer| layer.consumes().iter().any(|r| r == "warehouse.raw.orders"))
        .expect("canonical warehouse physical source");
    let ComposedSemantics::Resolved(semantics) = source.composed_semantics() else {
        panic!("typed source layer became unresolved");
    };
    semantics
}

#[test]
fn all_frozen_mandatory_expression_variants_have_direct_sql_and_dbt_parity() {
    let freeze: Value = serde_json::from_str(FREEZE).unwrap();
    let parser = dialect_from_name("postgresql").unwrap();
    let schema = RelationSchema::new(
        "warehouse.raw.orders",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("s", "VARCHAR(8)", "postgresql").unwrap(),
        ],
    )
    .unwrap();
    let direct_catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let mut confirmed = 0_usize;

    for cell in freeze["inventory_cells"].as_array().unwrap() {
        if cell["classification"] != "exact_required" {
            continue;
        }
        confirmed += 1;
        let id = cell["id"].as_str().unwrap();
        let sql = cell["sql_shape"]
            .as_str()
            .unwrap()
            .replace(" FROM t", " FROM warehouse.raw.orders");
        assert!(
            sql.contains("warehouse.raw.orders"),
            "{id}: no physical source in frozen exact SQL: {sql}"
        );

        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "direct",
            &input,
            "postgresql",
            parser.as_ref(),
        )];
        let mut direct = analyze_configured_inputs_with_catalog(&configured, &direct_catalog)
            .unwrap_or_else(|error| panic!("{id}: direct SQL {sql}: {error}"));

        let mut manifest: Value =
            serde_json::from_str(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
        let mut artifact: Value =
            serde_json::from_str(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
        manifest["nodes"]["model.demo.stg_orders"]["compiled_code"] = json!(sql);
        manifest["sources"]["source.demo.orders"]["columns"] = json!({
            "a":{"name":"a","data_type":"INTEGER"},
            "b":{"name":"b","data_type":"INTEGER"},
            "s":{"name":"s","data_type":"VARCHAR(8)"}
        });
        artifact["sources"]["source.demo.orders"]["columns"] = json!({
            "a":{"name":"a","type":"INTEGER","index":1},
            "b":{"name":"b","type":"INTEGER","index":2},
            "s":{"name":"s","type":"VARCHAR(8)","index":3}
        });
        let dbt_manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt_catalog = parse_dbt_catalog(&artifact.to_string()).unwrap();
        let mut dbt =
            analyze_dbt_artifacts(&dbt_manifest, &dbt_catalog, "postgresql", parser.as_ref())
                .unwrap_or_else(|error| panic!("{id}: compiled dbt SQL {sql}: {error}"));

        if cell["feature_id"] == "predicates.string_pattern" {
            let comparison_laws = [
                ComparisonAssumption::BinaryCollation,
                ComparisonAssumption::NoCharPadding,
            ];
            direct.declare_comparison_assumptions(&comparison_laws);
            dbt.declare_comparison_assumptions(&comparison_laws);
        }

        let expected = source_semantics(&direct);
        let actual = source_semantics(&dbt);
        assert_eq!(
            actual.output(),
            expected.output(),
            "{id}: output typed domain / bounds"
        );
        assert_eq!(
            actual.column_domains(),
            expected.column_domains(),
            "{id}: physical input domain"
        );
        assert_eq!(
            actual.condition_exactness(),
            expected.condition_exactness(),
            "{id}: false exactness / residual strength"
        );
        assert_eq!(
            actual
                .boolean_witnesses()
                .iter()
                .map(|w| w.witness())
                .collect::<Vec<_>>(),
            expected
                .boolean_witnesses()
                .iter()
                .map(|w| w.witness())
                .collect::<Vec<_>>(),
            "{id}: SQL TRUE and NOT TRUE obligation parity"
        );
    }
    assert_eq!(
        confirmed, 28,
        "mandatory exact feature inventory silently changed"
    );
}
