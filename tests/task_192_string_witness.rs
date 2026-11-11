//! TASK-192 portable pattern witnesses require explicit source and session laws.
//! Native DuckDB string bags are independently frozen in TASK-192. This file
//! proves the public typed row witness and the fail-closed nonportable cases.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, dialect_from_name, BooleanTruthCase,
    BooleanWitnessDirection, ComparisonAssumption, ComposedSemantics, ConfiguredSqlInput,
    RelationCatalog, SqlInput, ValueDomain,
};

const FREEZE: &str = include_str!("../docs/task-192-acceptance-freeze.json");

fn source_catalog(name_type: &str) -> RelationCatalog {
    let dialect = dialect_from_name("duckdb").expect("DDL parser");
    let ddl = SqlInput::inline(format!(
        "CREATE TABLE customers (id INTEGER, name {name_type})"
    ));
    let source = analyze_inputs(&[ddl], "duckdb", dialect.as_ref()).expect("source schema");
    RelationCatalog::from_schemas(source.source_schemas()).expect("catalog")
}

fn directions(
    sql: &str,
    dialect_name: &str,
    catalog: &RelationCatalog,
    laws: &[ComparisonAssumption],
) -> (bool, ValueDomain) {
    let dialect = dialect_from_name(dialect_name).expect("public SQL parser");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task192-original-pattern",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .expect("configured original SQL");
    bundle.declare_comparison_assumptions(laws);
    let layer = bundle.layers().last().expect("query");
    let ComposedSemantics::Resolved(composed) = layer.composed_semantics() else {
        panic!("original direct source must compose: {sql}");
    };
    let exact_witness = composed.boolean_witnesses().iter().any(|item| {
        matches!(
            item.witness().qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ) && matches!(
            item.witness().rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        )
    });
    (
        exact_witness,
        composed.output().columns()[0].domain().clone(),
    )
}

#[test]
fn original_safe_like_prefixes_require_both_binary_string_laws_on_all_13_parsers() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("original SQL");
    let variable = source_catalog("VARCHAR(64)");
    let fixed = source_catalog("CHAR(64)");
    let binary_laws = [
        ComparisonAssumption::BinaryCollation,
        ComparisonAssumption::NoCharPadding,
    ];
    let mut cells = 0;

    for id in ["88-C177-S020", "88-C177-S022", "88-C177-S025"] {
        let sql = freeze["cases"]
            .as_array()
            .expect("43 cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("frozen original")["sql"]
            .as_str()
            .expect("original SQL");
        for name in freeze["dialects"].as_array().expect("13 parsers") {
            let name = name.as_str().expect("parser name");
            for laws in [&[][..], &binary_laws[..1], &binary_laws[1..]] {
                assert!(
                    !directions(sql, name, &variable, laws).0,
                    "{id}/{name}: missing comparison law cannot certify LIKE"
                );
            }
            let (exact, domain) = directions(sql, name, &variable, &binary_laws);
            assert!(
                exact,
                "{id}/{name}: attested safe prefix has no typed TRUE/NOT TRUE witness"
            );
            assert!(
                matches!(domain, ValueDomain::Unbounded | ValueDomain::Unknown(_)),
                "{id}/{name}: filtering name cannot invent finite projected id"
            );
            assert!(
                !directions(sql, name, &fixed, &binary_laws).0,
                "{id}/{name}: CHAR padding remains unproved even with caller claims"
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 39);
}

#[test]
fn nonportable_string_pattern_families_cannot_be_exact_from_binary_attestation() {
    let freeze: Value = serde_json::from_str(FREEZE).expect("original SQL");
    let variable = source_catalog("VARCHAR(64)");
    let binary_laws = [
        ComparisonAssumption::BinaryCollation,
        ComparisonAssumption::NoCharPadding,
    ];
    let mut cells = 0;
    for id in [
        "88-C177-S021",
        "88-C177-S023",
        "88-C177-S024",
        "88-C177-S026",
        "88-C177-S038",
    ] {
        let sql = freeze["cases"]
            .as_array()
            .expect("original cases")
            .iter()
            .find(|case| case["id"] == id)
            .expect("original nonportable SQL")["sql"]
            .as_str()
            .expect("original SQL");
        for name in freeze["dialects"].as_array().expect("13 parser dialects") {
            let name = name.as_str().expect("dialect");
            assert!(
                !directions(sql, name, &variable, &binary_laws).0,
                "{id}/{name}: session-dependent pattern/collation cannot borrow binary LIKE proof"
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 65);
}
