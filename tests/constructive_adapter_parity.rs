//! Canonical witnesses are source independent: dbt compiled SQL and direct SQL share proof facts.

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, local_constructive_witnesses,
    parse_dbt_catalog, parse_dbt_manifest, physical_joint_source_plan, ComposedSemantics,
    ConfiguredSqlInput, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessOperator,
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
            "warehouse.raw.orders",
            vec![
                SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").expect("id"),
                SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").expect("amount"),
            ],
        )
        .expect("source schema")
        .with_source_kind(kind);
        let catalog = RelationCatalog::from_schemas(&[source]).expect("catalog");
        let sql_input = SqlInput::inline(SQL);
        let bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "direct",
                &sql_input,
                "postgresql",
                &dialect,
            )],
            &catalog,
        )
        .expect("analysis");
        let ComposedSemantics::Resolved(ref semantics) = bundle.layers()[0].composed_semantics()
        else {
            panic!("resolved");
        };
        let witness = local_constructive_witnesses(semantics)
            .into_iter()
            .find(|w| w.operator() == WitnessOperator::Boolean)
            .expect("typed witness");
        if let Some(ref expected) = baseline {
            assert_eq!(witness.qualifying(), expected);
        } else {
            baseline = Some(witness.qualifying().clone());
        }
    }
}
#[test]
fn canonical_joint_source_proofs_are_independent_of_schema_adapter_provenance() {
    use sql_semantic_protocol::SchemaSourceKind;
    let dialect = PostgreSqlDialect {};
    let inputs = [
        SqlInput::inline("SELECT amount FROM warehouse.raw.orders WHERE amount > 10"),
        SqlInput::inline("SELECT amount FROM warehouse.raw.orders WHERE amount < 100"),
    ];
    let configured = inputs
        .iter()
        .enumerate()
        .map(|(index, source)| {
            ConfiguredSqlInput::new(
                if index == 0 { "positive" } else { "bounded" },
                source,
                "postgresql",
                &dialect,
            )
        })
        .collect::<Vec<_>>();
    let mut baseline = None;
    for kind in [
        SchemaSourceKind::DbtCatalog,
        SchemaSourceKind::DbtManifest,
        SchemaSourceKind::ExternalMetadata,
    ] {
        let schema = RelationSchema::new(
            "warehouse.raw.orders",
            vec![
                SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").expect("id"),
                SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").expect("amount"),
            ],
        )
        .expect("schema")
        .with_source_kind(kind);
        let catalog = RelationCatalog::from_schemas(&[schema]).expect("catalog");
        let bundle =
            analyze_configured_inputs_with_catalog(&configured, &catalog).expect("SQL analysis");
        let goals = [(bundle.layers()[0].id(), 3), (bundle.layers()[1].id(), 3)];
        let plan = physical_joint_source_plan(&bundle, &goals);
        assert!(
            matches!(plan.outcome(), WitnessDirection::Feasible(_)),
            "{kind:?}: {plan:?}"
        );
        if let Some(expected) = &baseline {
            assert_eq!(
                &plan, expected,
                "schema provenance must not change canonical proof"
            );
        } else {
            baseline = Some(plan);
        }
    }
}

#[test]
fn joined_physical_populations_do_not_depend_on_schema_adapter_provenance() {
    use sql_semantic_protocol::SchemaSourceKind;
    let dialect = PostgreSqlDialect {};
    let inputs = [
        SqlInput::inline("CREATE TABLE stage AS SELECT a,k FROM l"),
        SqlInput::inline("CREATE TABLE mart AS SELECT a,k FROM r"),
        SqlInput::inline("SELECT s.a FROM stage s JOIN mart m ON s.k=m.k"),
    ];
    let configured = inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let label = match index {
                0 => "stage",
                1 => "mart",
                _ => "joined",
            };
            ConfiguredSqlInput::new(label, input, "postgresql", &dialect)
        })
        .collect::<Vec<_>>();
    let mut baseline = None;
    for kind in [
        SchemaSourceKind::DbtCatalog,
        SchemaSourceKind::DbtManifest,
        SchemaSourceKind::ExternalMetadata,
    ] {
        let schemas = ["l", "r", "stage", "mart"]
            .into_iter()
            .map(|relation| {
                RelationSchema::new(
                    relation,
                    ["a", "k"]
                        .into_iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(column, "INTEGER", "postgresql")
                                .expect("integer")
                        })
                        .collect(),
                )
                .expect("schema")
                .with_source_kind(kind)
            })
            .collect::<Vec<_>>();
        let catalog = RelationCatalog::from_schemas(&schemas).expect("catalog");
        let bundle =
            analyze_configured_inputs_with_catalog(&configured, &catalog).expect("analyze");
        let proof = physical_joint_source_plan(&bundle, &[(bundle.layers()[2].id(), 4)]);
        assert!(
            matches!(proof.outcome(), WitnessDirection::Feasible(_)),
            "{kind:?}: {proof:?}"
        );
        if let Some(expected) = &baseline {
            assert_eq!(
                &proof, expected,
                "{kind:?}: source kind must not alter join law"
            );
        } else {
            baseline = Some(proof);
        }
    }
}
