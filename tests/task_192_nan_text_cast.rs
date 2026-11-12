//! TASK-192 original S040: an untyped text spelling of NaN cannot certify
//! a numeric implicit cast or a warehouse's NaN equality law.
//! DuckDB source oracles remain separate from parser-independent guarantees.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, analyze_sql, dialect_from_name,
    to_json, ComparisonAssumption, ComposedSemantics, ConditionExactnessStatus, ConfiguredSqlInput,
    RelationCatalog, SqlInput, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn original_sql() -> String {
    let frozen: Value = serde_json::from_str(FREEZE).expect("fixed TASK-192 matrix");
    frozen["cases"]
        .as_array()
        .expect("43 original cases")
        .iter()
        .find(|row| row["id"] == "88-C177-S040")
        .expect("unaltered S040")["sql"]
        .as_str()
        .expect("original SQL string")
        .to_owned()
}

fn signed_and_float_catalog() -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("DDL dialect");
    let source = analyze_inputs(
        &[SqlInput::inline(
            "CREATE TABLE orders (id INTEGER, amount DOUBLE)",
        )],
        "duckdb",
        dialect.as_ref(),
    )
    .expect("independent source schema");
    RelationCatalog::from_schemas(source.source_schemas()).expect("catalog")
}

#[test]
fn all_13_untyped_original_nan_text_comparisons_are_unapproved_residuals() {
    let sql = original_sql();
    let original: Value = serde_json::from_str(FREEZE).expect("original dialects");
    for name in original["dialects"].as_array().expect("13 parser dialects") {
        let name = name.as_str().expect("public dialect");
        let dialect = dialect_from_name(name).expect("public parser");
        let parsed = analyze_sql(&sql, name, dialect.as_ref()).expect("original SQL parses");
        let wire: Value = serde_json::from_str(&to_json(&parsed)).expect("public JSON document");
        let statement = &wire["inputs"][0]["statements"][0];
        assert_eq!(statement["condition_exactness"]["status"], "residual");
        let residuals = statement["condition_exactness"]["residual_conditions"]
            .as_array()
            .expect("named unproved implicit cast");
        assert_eq!(residuals.len(), 1, "S040/{name}: unique missing law");
        assert_eq!(residuals[0]["reason"], "comparison_semantics");
        assert_eq!(residuals[0]["identity"], "where:nan_implicit_cast");
        assert_eq!(
            statement["output"]["columns"][0]["domain"]["kind"],
            "unbounded"
        );
    }
}

#[test]
fn declaring_collation_padding_float_and_time_zone_cannot_attest_nan_cast() {
    let sql = original_sql();
    let input = SqlInput::inline(&sql);
    let original: Value = serde_json::from_str(FREEZE).expect("original dialects");
    let catalog = signed_and_float_catalog();
    for name in original["dialects"].as_array().expect("13 parser dialects") {
        let name = name.as_str().expect("public dialect");
        let dialect = dialect_from_name(name).expect("public parser");
        let configured = [ConfiguredSqlInput::new(
            "s040-original",
            &input,
            name,
            dialect.as_ref(),
        )];
        let mut bundle = analyze_configured_inputs_with_catalog(&configured, &catalog)
            .expect("typed original S040");
        bundle.declare_comparison_assumptions(&[
            ComparisonAssumption::BinaryCollation,
            ComparisonAssumption::NoCharPadding,
            ComparisonAssumption::NoNan,
            ComparisonAssumption::SignedZeroEquivalent,
            ComparisonAssumption::SessionTimeZone,
        ]);
        let ComposedSemantics::Resolved(semantics) = bundle
            .layers()
            .last()
            .expect("original query layer")
            .composed_semantics()
        else {
            panic!("S040/{name}: composed source must resolve");
        };
        assert_eq!(
            semantics.condition_exactness().status(),
            ConditionExactnessStatus::Residual,
            "S040/{name}: explicit runtime implicit-cast behavior is unproved"
        );
        assert!(
            matches!(
                semantics.output().columns()[0].domain(),
                ValueDomain::Unbounded
            ),
            "S040/{name}: filtering amount cannot bound id"
        );
    }
}
