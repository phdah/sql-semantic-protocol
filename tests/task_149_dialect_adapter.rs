//! TASK-149: dialect/parser boundaries and equivalent source-schema adapters.
//!
//! Parser classification is not a native vendor execution claim. Physical
//! identity and row-state parity are proved separately from the metadata source.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan,
    ProtocolStatement, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};

const DIALECTS: [&str; 13] = [
    "generic",
    "mysql",
    "postgresql",
    "hive",
    "sqlite",
    "snowflake",
    "redshift",
    "mssql",
    "clickhouse",
    "bigquery",
    "ansi",
    "duckdb",
    "databricks",
];
const TARGET: &str = "warehouse.analytics.stg_orders";
const SQL: &str = "UPDATE warehouse.analytics.stg_orders SET amount = amount + 2 WHERE id = 2";

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        TARGET,
        vec![
            SchemaColumn::from_sql_type("id", "BIGINT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("amount", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap()])
    .unwrap()
}

fn dbt_catalog() -> RelationCatalog {
    let manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let analyzed =
        analyze_dbt_artifacts(&manifest, &catalog, "postgresql", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(analyzed.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let known = RelationCatalog::new(&[TARGET]).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task149parity
name: Equivalent single target update metadata
version: 1.0.0
status: active
schema:
  - name: stg_orders
    physicalName: warehouse.analytics.stg_orders
    properties:
      - name: id
        physicalType: BIGINT
      - name: amount
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    RelationCatalog::from_schemas(contract.source_schemas()).unwrap()
}

fn initial() -> PhysicalWriteState {
    PhysicalWriteState::new(
        TARGET,
        [(Some(1), 10), (Some(2), 20), (None, 30), (Some(2), 20)]
            .into_iter()
            .map(|(id, amount)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".into(),
                        id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    ),
                    ("amount".into(), LiteralValue::Number(amount.to_string())),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn execute(catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let sql = SqlInput::inline(SQL);
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = ConfiguredSqlInput::new("task149", &sql, "postgresql", dialect.as_ref());
    let bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    physical_ordered_write_state_plan(&bundle, &[initial()])
}

#[test]
fn direct_and_dbt_catalogs_construct_same_complete_mutation_population() {
    let direct = execute(&direct_catalog());
    let dbt = execute(&dbt_catalog());
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = &direct
    else {
        panic!("expected full physical mutation proof: {direct:?}");
    };
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].before(), &initial());
    assert_eq!(transitions[0].affected().updated(), 2);
    assert_eq!(final_states.len(), 1);
    let expected = PhysicalWriteState::new(
        TARGET,
        [(Some(1), 10), (Some(2), 22), (None, 30), (Some(2), 22)]
            .into_iter()
            .map(|(id, amount)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    (
                        "id".into(),
                        id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    ),
                    ("amount".into(), LiteralValue::Number(amount.to_string())),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    assert_eq!(transitions[0].after(), &expected);
    assert_eq!(
        direct, dbt,
        "dbt schema provenance cannot change SQL write effects"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn odcs_and_sql_schema_have_identical_complete_poststate() {
    assert_eq!(execute(&direct_catalog()), execute(&odcs_catalog()));
}

#[test]
fn every_exposed_dialect_classifies_single_target_mutations_without_silent_rewrite() {
    let forms = [
        (
            "UPDATE target SET score = 40 WHERE id = 1",
            WriteKind::Update,
            false,
            false,
        ),
        (
            "UPDATE target SET score = score + 5 WHERE id = 2",
            WriteKind::Update,
            false,
            false,
        ),
        (
            "UPDATE target SET score = DEFAULT WHERE id = 1",
            WriteKind::Update,
            false,
            true,
        ),
        (
            "DELETE FROM target WHERE id = NULL",
            WriteKind::Delete,
            false,
            false,
        ),
        (
            "UPDATE target SET score = 40 WHERE id = 1 RETURNING id,score",
            WriteKind::Update,
            true,
            false,
        ),
        (
            "DELETE FROM target WHERE id = 2 RETURNING id,score",
            WriteKind::Delete,
            true,
            false,
        ),
    ];
    let mut cells = 0;
    let mut classified = [0_usize; 6];
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for (index, (sql, kind, returning, default)) in forms.iter().enumerate() {
            cells += 1;
            let Ok(bundle) = analyze_inputs(&[SqlInput::inline(*sql)], dialect, parser.as_ref())
            else {
                continue;
            };
            match &bundle.inputs()[0].statements()[0] {
                ProtocolStatement::Query(query) => {
                    let write = query
                        .write()
                        .expect("UPDATE/DELETE may not silently become read");
                    assert_eq!(write.kind(), *kind, "{dialect} / {sql}");
                    assert_eq!(
                        !write.returning_columns().is_empty(),
                        *returning,
                        "{dialect}"
                    );
                    assert_eq!(
                        write.assignments().iter().any(|a| a.is_default()),
                        *default,
                        "{dialect}"
                    );
                    classified[index] += 1;
                }
                ProtocolStatement::Unsupported(_) => {}
                _ => panic!("{dialect}: mutation input has unexpected protocol statement"),
            }
        }
    }
    assert_eq!(cells, 78);
    assert!(
        classified.iter().all(|n| *n > 0),
        "each mandatory family needs a canonical parser: {classified:?}"
    );
}

fn frozen_cells() -> serde_json::Value {
    serde_json::from_str(include_str!("../docs/task-149-acceptance-freeze.json")).unwrap()
}

fn adapted_sql(case: &serde_json::Value) -> String {
    // These frozen single-target forms have no other relations to rename.
    case["sql"].as_str().unwrap()
        .replace("target", TARGET)
        .replace("score", "amount")
}

fn frozen_initial(case: &serde_json::Value) -> PhysicalWriteState {
    PhysicalWriteState::new(
        TARGET,
        case["target_before"].as_array().unwrap().iter().map(|row| {
            let id = row["id"].as_i64()
                .map_or(LiteralValue::Null, |id| LiteralValue::Number(id.to_string()));
            let amount = LiteralValue::Number(row["score"].as_i64().unwrap().to_string());
            PhysicalWriteRow::new(BTreeMap::from([
                ("id".into(), id), ("amount".into(), amount)
            ])).unwrap()
        }).collect(),
    ).unwrap()
}

fn frozen_plan(
    catalog: &RelationCatalog,
    case: &serde_json::Value,
) -> PhysicalWriteStatePlan {
    use sql_semantic_protocol::{
        ColumnValueKind, ColumnValueSource, ConstraintEnforcement,
        ConstraintEvidence, ConstraintProvenance, ConstraintSourceKind,
        ConstraintValue, RelationConstraint, RelationConstraintSet,
    };
    let sql_text = adapted_sql(case);
    let sql = SqlInput::inline(&sql_text);
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = ConfiguredSqlInput::new(
        "task149-freeze-parity", &sql, "postgresql", dialect.as_ref()
    );
    let mut bundle = analyze_configured_inputs_with_catalog(&[configured], catalog).unwrap();
    let evidence = vec![ConstraintEvidence::new(
        ConstraintProvenance::new(
            ConstraintSourceKind::ExternalMetadata, "task149-parity"
        ).unwrap(),
        ConstraintEnforcement::Enforced,
    )];
    if case["family"] == "update_default" {
        let default = ColumnValueSource::new(
            "amount", ColumnValueKind::Default,
            Some(ConstraintValue::Integer(7)), evidence.clone()
        ).unwrap();
        bundle.enrich_relation_constraints(&[
            RelationConstraintSet::new(TARGET, vec![]).unwrap()
                .with_column_value_sources(vec![default])
        ]);
    } else if case["family"] == "enforced_primary_key_conflict" {
        bundle.enrich_relation_constraints(&[
            RelationConstraintSet::new(TARGET, vec![
                RelationConstraint::primary_key(vec!["id".into()], evidence).unwrap()
            ]).unwrap()
        ]);
    }
    physical_ordered_write_state_plan(&bundle, &[frozen_initial(case)])
}

#[test]
fn all_frozen_mutation_cases_have_identical_direct_and_dbt_schema_evidence() {
    let freeze = frozen_cells();
    assert_eq!(freeze["version"], 2);
    let mut count = 0_usize;
    for case in freeze["cases"].as_array().unwrap() {
        let direct = frozen_plan(&direct_catalog(), case);
        let dbt = frozen_plan(&dbt_catalog(), case);
        assert_eq!(direct, dbt, "{}",case["id"]);
        match case["proof"].as_str().unwrap() {
            "impossible" => assert!(
                matches!(direct, PhysicalWriteStatePlan::Impossible { .. }),"{}",case["id"]
            ),
            "residual_missing_attestation" => assert!(
                matches!(direct, PhysicalWriteStatePlan::Residual { .. }),"{}",case["id"]
            ),
            _ => assert!(
                matches!(direct, PhysicalWriteStatePlan::Feasible { .. }),"{}",case["id"]
            ),
        }
        count += 1;
    }
    assert_eq!(count, 13);
}

#[cfg(feature = "odcs")]
#[test]
fn all_frozen_mutation_cases_have_identical_odcs_schema_evidence() {
    let freeze = frozen_cells();
    let mut count = 0_usize;
    for case in freeze["cases"].as_array().unwrap() {
        assert_eq!(
            frozen_plan(&direct_catalog(), case),
            frozen_plan(&odcs_catalog(), case),
            "{}: metadata provenance cannot affect canonical mutation",
            case["id"],
        );
        count += 1;
    }
    assert_eq!(count, 13);
}

#[test]
fn vendor_mutation_forms_cannot_be_misclassified_as_single_target_exact() {
    let unsupported = [
        "UPDATE target SET score = source.score FROM source WHERE target.id = source.id",
        "DELETE FROM target USING source WHERE target.id = source.id",
        "DELETE FROM target WHERE id = 1 ORDER BY id LIMIT 1",
        "UPDATE target SET score = 40 WHERE id = 1 RETURNING *",
        "DELETE FROM target WHERE id = 1 RETURNING id AS alias_id",
    ];
    let mut cells = 0_usize;
    for dialect in DIALECTS {
        let parser = dialect_from_name(dialect).unwrap();
        for sql in unsupported {
            let result = analyze_inputs(&[SqlInput::inline(sql)], dialect, parser.as_ref());
            if let Ok(bundle) = result {
                assert!(
                    matches!(bundle.inputs()[0].statements()[0],ProtocolStatement::Unsupported(_)),
                    "{dialect}: cannot certify unproved mutation form: {sql}",
                );
            }
            cells += 1;
        }
    }
    assert_eq!(cells, 65);
}
