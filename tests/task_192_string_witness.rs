//! TASK-192 portable pattern witnesses require explicit source and session laws.
//! Native DuckDB string bags are independently frozen in TASK-192. This file
//! proves the public typed row witness and the fail-closed nonportable cases.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, analyze_sql, dialect_from_name, to_json,
    BooleanTruthCase,
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

#[test]
fn original_trailing_space_string_equality_requires_explicit_comparison_laws() {
    // The original S039 query compares a VARCHAR to a padded literal.
    // A binary, non-padding source law must be declared before a row-local
    // equality witness can be Exact. This does not attest native vendors.
    let freeze: Value = serde_json::from_str(FREEZE).expect("original SQL");
    let sql = freeze["cases"]
        .as_array()
        .expect("43 cases")
        .iter()
        .find(|case| case["id"] == "88-C177-S039")
        .expect("original S039")["sql"]
        .as_str()
        .expect("original SQL");
    let variable = source_catalog("VARCHAR(64)");
    let fixed = source_catalog("CHAR(64)");
    let laws = [
        ComparisonAssumption::BinaryCollation,
        ComparisonAssumption::NoCharPadding,
    ];
    for name in freeze["dialects"].as_array().expect("13 dialects") {
        let name = name.as_str().expect("dialect");
        assert!(
            !directions(sql, name, &variable, &[]).0,
            "S039/{name}: no implicit binary comparison environment"
        );
        assert!(
            !directions(sql, name, &variable, &laws[..1]).0,
            "S039/{name}: binary collation does not attest CHAR trimming"
        );
        assert!(
            directions(sql, name, &variable, &laws).0,
            "S039/{name}: attested VARCHAR string equality should have a typed witness"
        );
        assert!(
            !directions(sql, name, &fixed, &laws).0,
            "S039/{name}: fixed CHAR padding remains an unproved source law"
        );
    }
}

#[test]
fn original_untyped_s039_never_assumes_fixed_char_padding_is_absent() {
    // No schema is supplied: this source could be CHAR or VARCHAR. Binary
    // collation cannot by itself justify the original 'abc ' equality.
    let freeze: Value = serde_json::from_str(FREEZE).expect("original case inventory");
    let sql = freeze["cases"]
        .as_array()
        .expect("43 original SQLs")
        .iter()
        .find(|case| case["id"] == "88-C177-S039")
        .expect("S039 original")["sql"]
        .as_str()
        .expect("unaltered SQL");
    for name in freeze["dialects"].as_array().expect("13 parser names") {
        let name = name.as_str().expect("dialect");
        let parser = dialect_from_name(name).expect("public parser");
        let protocol = analyze_sql(sql, name, parser.as_ref()).expect("S039 parser");
        let wire: Value =
            serde_json::from_str(&to_json(&protocol)).expect("canonical public document");
        let exactness = &wire["inputs"][0]["statements"][0]["condition_exactness"];
        assert_eq!(
            exactness["status"], "conditional",
            "S039/{name}: source-free parser cannot attest a warehouse string law"
        );
        let requirements = exactness["comparison_assumptions"]
            .as_array()
            .expect("explicit conditional laws");
        let mut actual = requirements
            .iter()
            .map(|requirement| {
                assert_eq!(
                    requirement["declared"], false,
                    "S039/{name}: no silently attested warehouse session"
                );
                assert_eq!(requirement["clause"], "where");
                assert!(
                    requirement["identity"]
                        .as_str()
                        .is_some_and(|identity| identity.contains("customers.name")),
                    "S039/{name}: comparison assumption must name its source column"
                );
                requirement["name"].as_str().expect("named assumption")
            })
            .collect::<Vec<_>>();
        actual.sort_unstable();
        assert_eq!(
            actual,
            vec!["binary_collation", "no_char_padding"],
            "S039/{name}: independently required comparison laws"
        );
    }
}
