//! TASK-179 source-typed vendor law witness boundary tests.
//!
//! Original SQL spelling and conditional comparison laws are preserved. An
//! external session assumption is never inferred from a 13-dialect parser hit.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_inputs, analyze_sql, dialect_from_name,
    physical_duckdb_sql_transaction_plan_with_laws, to_json, BooleanTruthCase,
    BooleanWitnessDirection, ComparisonAssumption, ComposedSemantics, ConfiguredSqlInput,
    PhysicalScriptCatalog, PhysicalTransactionScriptProof, PhysicalTransactionSessionLaw,
    RelationCatalog, SqlInput, ValueDomain,
};

const MATRIX: &str = include_str!("../docs/task-179-variant-matrix-v2.json");
const ORIGINAL: &str = include_str!("../docs/task-177-parser-samples.json");

fn source_catalog(ty: &str) -> RelationCatalog {
    let parser = dialect_from_name("duckdb").expect("supported source DDL");
    let source = SqlInput::inline(format!(
        "CREATE TABLE customers (id INTEGER, name {ty})"
    ));
    let analyzed = analyze_inputs(&[source], "duckdb", parser.as_ref())
        .expect("authoritative source DDL");
    RelationCatalog::from_schemas(analyzed.source_schemas()).expect("typed catalog")
}

fn exact_row_coupled_witness(
    sql: &str,
    dialect: &str,
    catalog: &RelationCatalog,
    laws: &[ComparisonAssumption],
) -> (bool, ValueDomain) {
    let parser = dialect_from_name(dialect).expect("supported parser");
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task179-vendor-comparison",
        &input,
        dialect,
        parser.as_ref(),
    )];
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, catalog)
        .unwrap_or_else(|error| panic!("{dialect}: source-typed comparison: {error}"));
    bundle.declare_comparison_assumptions(laws);
    let ComposedSemantics::Resolved(composed) = bundle
        .layers()
        .last()
        .expect("single SQL query")
        .composed_semantics()
    else {
        panic!("{dialect}: missing resolved SQL semantics");
    };
    let exact = composed.boolean_witnesses().iter().any(|entry| {
        matches!(
            entry.witness().qualifying(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::True)
        ) && matches!(
            entry.witness().rejected(),
            BooleanWitnessDirection::Exact(BooleanTruthCase::NotTrue)
        )
    });
    (exact, composed.output().columns()[0].domain().clone())
}

fn original_sql<'a>(fixture: &'a Value, id: &str) -> &'a str {
    fixture["cases"]
        .as_array()
        .expect("original cases")
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("missing source case {id}"))["sql"]
        .as_str()
        .expect("original SQL")
}

#[test]
fn no_unattested_collation_padding_or_pattern_can_be_promoted_to_exact() {
    let matrix: Value = serde_json::from_str(MATRIX).expect("vendor law freeze");
    let original: Value = serde_json::from_str(ORIGINAL).expect("SQL source");
    let variable = source_catalog("VARCHAR(64)");
    let fixed = source_catalog("CHAR(64)");
    let declared_laws = [
        ComparisonAssumption::BinaryCollation,
        ComparisonAssumption::NoCharPadding,
    ];
    let names = matrix["dialects"].as_array().expect("13 parser dialects");
    let mut checked = 0;
    for dialect in names {
        let name = dialect.as_str().expect("dialect name");
        for id in [
            "88-C177-S021",
            "88-C177-S023",
            "88-C177-S024",
            "88-C177-S026",
            "88-C177-S038",
        ] {
            let sql = original_sql(&original, id);
            assert!(
                !exact_row_coupled_witness(sql, name, &variable, &declared_laws).0,
                "{id}/{name}: binary attestation cannot certify a locale/regex law"
            );
            checked += 1;
        }
        let sql = original_sql(&original, "88-C177-S039");
        assert!(
            !exact_row_coupled_witness(sql, name, &variable, &[]).0,
            "{name}: missing locale/padding attestation"
        );
        assert!(
            !exact_row_coupled_witness(sql, name, &variable, &declared_laws[..1]).0,
            "{name}: binary collation alone does not establish padding law"
        );
        let (exact, output_domain) =
            exact_row_coupled_witness(sql, name, &variable, &declared_laws);
        assert!(exact, "{name}: attested VARCHAR witness lost");
        assert!(
            matches!(output_domain, ValueDomain::Unbounded | ValueDomain::Unknown(_)),
            "{name}: filtered string cannot prove finite projected ID domain"
        );
        assert!(
            !exact_row_coupled_witness(sql, name, &fixed, &declared_laws).0,
            "{name}: fixed-width CHAR still requires an independent padding law"
        );
    }
    assert_eq!(checked, 65);
}

#[test]
fn untyped_string_equality_requires_explicit_undeclared_session_laws() {
    let fixture: Value = serde_json::from_str(MATRIX).expect("task freeze");
    let original: Value = serde_json::from_str(ORIGINAL).expect("original SQL");
    let sql = original_sql(&original, "88-C177-S039");
    for dialect in fixture["dialects"].as_array().expect("13 dialects") {
        let name = dialect.as_str().expect("dialect");
        let parser = dialect_from_name(name).expect("parser");
        let protocol = analyze_sql(sql, name, parser.as_ref())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let wire: Value = serde_json::from_str(&to_json(&protocol)).expect("public JSON");
        let claim = &wire["inputs"][0]["statements"][0]["condition_exactness"];
        assert_eq!(claim["status"], "conditional");
        let requirements = claim["comparison_assumptions"]
            .as_array()
            .expect("named source session requirements");
        let mut law_names = requirements
            .iter()
            .map(|item| {
                assert_eq!(item["declared"], false, "{name}: assumption not attested");
                item["name"].as_str().expect("named source law")
            })
            .collect::<Vec<_>>();
        law_names.sort_unstable();
        assert_eq!(law_names, ["binary_collation", "no_char_padding"]);
    }
}

#[test]
fn vendor_transaction_and_cross_session_laws_remain_residual() {
    let empty = PhysicalScriptCatalog {
        states: vec![],
        schemas: vec![],
    };
    for law in [
        PhysicalTransactionSessionLaw::TemporaryTableLifetime,
        PhysicalTransactionSessionLaw::DeferredForeignKeyChecks,
        PhysicalTransactionSessionLaw::ConcurrentIsolation,
        PhysicalTransactionSessionLaw::VendorImplicitDdlCommit,
        PhysicalTransactionSessionLaw::VendorTransactionOptions,
    ] {
        let proof = physical_duckdb_sql_transaction_plan_with_laws(
            &["BEGIN", "COMMIT"],
            &empty,
            &[law],
        );
        match proof {
            PhysicalTransactionScriptProof::Residual {
                step_index: 0,
                reason,
            } => {
                assert!(reason.contains(law.as_str()));
                assert!(reason.contains("TASK-163"));
            }
            other => panic!("unsupported {law:?} cannot become exact: {other:?}"),
        }
    }
}
