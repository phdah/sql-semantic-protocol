//! TASK-152: classify all exposed parser spellings and source-independent
//! metadata parity for a complete three-write program.

use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    CheckCompareOperator, ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence,
    ConstraintProvenance, ConstraintSourceKind, ConstraintValue, LiteralValue, PhysicalWriteRow,
    PhysicalWriteState, PhysicalWriteStatePlan, ProtocolStatement, RelationCatalog,
    RelationConstraint, RelationConstraintSet, RelationSchema, SchemaColumn, SqlInput, WriteKind,
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
const SOURCE: &str = "warehouse.raw.orders";
const SCRIPT: [&str; 3] = [
    "INSERT INTO target (id,parent_id,score) VALUES (1,7,10),(2,7,20)",
    "UPDATE target SET score = score + 5 WHERE id = 1",
    "DELETE FROM target WHERE id = 2",
];

fn expected() -> serde_json::Value {
    serde_json::from_str(include_str!("../docs/task-152-acceptance-freeze.json")).unwrap()
}

#[test]
fn frozen_shared_write_syntax_classifies_all_thirteen_dialects() {
    let freeze = expected();
    assert_eq!(freeze["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(freeze["parser_sql"].as_array().unwrap().len(), 3);
    let kinds = [WriteKind::Append, WriteKind::Update, WriteKind::Delete];
    let mut actual = BTreeMap::<String, Vec<String>>::new();
    for dialect in DIALECTS {
        let engine = dialect_from_name(dialect).unwrap();
        let classifications = SCRIPT
            .iter()
            .zip(kinds)
            .map(|(sql, kind)| {
                match analyze_inputs(&[SqlInput::inline(*sql)], dialect, engine.as_ref()) {
                    Err(_) => "parser_boundary",
                    Ok(bundle) => match bundle.inputs()[0].statements().first().unwrap() {
                        ProtocolStatement::Unsupported(_) => "canonical_unsupported",
                        ProtocolStatement::Query(query)
                            if query.write().is_some_and(|w| w.kind() == kind) =>
                        {
                            "parsed_canonical"
                        }
                        ProtocolStatement::Query(_) => "canonical_unsupported",
                        _ => "canonical_unsupported",
                    },
                }
                .to_string()
            })
            .collect::<Vec<_>>();
        actual.insert(dialect.to_string(), classifications.clone());
        let frozen = freeze["dialect_expected"][dialect].as_array().unwrap();
        assert_eq!(
            frozen.len(),
            classifications.len(),
            "{dialect}: all statements classified"
        );
        for (index, (classification, expected)) in classifications.iter().zip(frozen).enumerate() {
            assert_eq!(
                Some(classification.as_str()),
                expected.as_str(),
                "{dialect}: frozen parser/canonical cell {index}; observed={actual:?}"
            );
        }
    }
    assert_eq!(actual.values().map(Vec::len).sum::<usize>(), 39);
}

fn direct_catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[TARGET, SOURCE].map(|relation| {
        RelationSchema::new(
            relation,
            ["id", "amount"]
                .into_iter()
                .map(|column| {
                    SchemaColumn::from_sql_type(
                        column,
                        if relation == TARGET && column == "id" {
                            "BIGINT"
                        } else {
                            "INTEGER"
                        },
                        "postgresql",
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap()
    }))
    .unwrap()
}

fn dbt_catalog() -> RelationCatalog {
    let manifest = parse_dbt_manifest(include_str!("fixtures/dbt/manifest-v12.json")).unwrap();
    let catalog = parse_dbt_catalog(include_str!("fixtures/dbt/catalog-v1.json")).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let dbt = analyze_dbt_artifacts(&manifest, &catalog, "postgresql", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(dbt.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;

    let known = RelationCatalog::new(&[TARGET, SOURCE]).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task152parity
name: Task 152 equivalent ordered write schema evidence
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
  - name: orders
    physicalName: warehouse.raw.orders
    properties:
      - name: id
        physicalType: INTEGER
      - name: amount
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    RelationCatalog::from_schemas(contract.source_schemas()).unwrap()
}

fn evidence() -> Vec<ConstraintEvidence> {
    vec![ConstraintEvidence::new(
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task152-parity")
            .unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn state_for(relation: &str, rows: &[(i64, i64)]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        relation,
        rows.iter()
            .map(|(id, amount)| {
                PhysicalWriteRow::new(BTreeMap::from([
                    ("id".to_string(), LiteralValue::Number(id.to_string())),
                    (
                        "amount".to_string(),
                        LiteralValue::Number(amount.to_string()),
                    ),
                ]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn state(rows: &[(i64, i64)]) -> PhysicalWriteState {
    state_for(TARGET, rows)
}

fn script() -> [String; 3] {
    [
        format!("INSERT INTO {TARGET} (id,amount) VALUES (1,10),(2,20)"),
        format!("UPDATE {TARGET} SET amount = amount + 5 WHERE id = 1"),
        format!("DELETE FROM {TARGET} WHERE id = 2"),
    ]
}

fn plan(catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let script = script();
    let sqls = script.iter().map(SqlInput::inline).collect::<Vec<_>>();
    let ids = (0..sqls.len())
        .map(|n| format!("task152-parity-{n}"))
        .collect::<Vec<_>>();
    let dialect = dialect_from_name("postgresql").unwrap();
    let configured = sqls
        .iter()
        .zip(&ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "postgresql", dialect.as_ref()))
        .collect::<Vec<_>>();
    let mut bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    bundle.enrich_relation_constraints(&[
        RelationConstraintSet::new(
            SOURCE,
            vec![RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap()],
        )
        .unwrap(),
        RelationConstraintSet::new(
            TARGET,
            vec![
                RelationConstraint::primary_key(vec!["id".into()], evidence()).unwrap(),
                RelationConstraint::foreign_key(
                    vec!["id".into()],
                    SOURCE,
                    vec!["id".into()],
                    evidence(),
                )
                .unwrap(),
                RelationConstraint::check_comparison(
                    "amount",
                    CheckCompareOperator::Gte,
                    ConstraintValue::Integer(0),
                    evidence(),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    ]);
    physical_ordered_write_state_plan(
        &bundle,
        &[state(&[]), state_for(SOURCE, &[(1, 0), (2, 0)])],
    )
}

#[test]
fn direct_dbt_and_odcs_equivalent_schema_evidence_keep_the_same_state_proof() {
    let direct = plan(&direct_catalog());
    let PhysicalWriteStatePlan::Feasible {
        transitions,
        final_states,
    } = &direct
    else {
        panic!("direct sequential write proof must be Exact: {direct:?}");
    };
    assert_eq!(transitions.len(), 3);
    assert_eq!(transitions[0].after(), &state(&[(1, 10), (2, 20)]));
    assert_eq!(transitions[1].before(), transitions[0].after());
    assert_eq!(transitions[1].after(), &state(&[(1, 15), (2, 20)]));
    assert_eq!(transitions[2].before(), transitions[1].after());
    assert_eq!(transitions[2].after(), &state(&[(1, 15)]));
    assert_eq!(
        final_states.as_slice(),
        &[state_for(SOURCE, &[(1, 0), (2, 0)]), state(&[(1, 15)])]
    );
    assert_eq!(direct, plan(&dbt_catalog()), "dbt catalog parity");
    #[cfg(feature = "odcs")]
    assert_eq!(direct, plan(&odcs_catalog()), "ODCS schema parity");
}
