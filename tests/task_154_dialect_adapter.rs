//! TASK-154 dialect/parser boundaries and source-independent direct/dbt/ODCS proof.
//! Parser acceptance is not evidence of native engine execution semantics.

use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, analyze_inputs,
    dialect_from_name, parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan,
    ConfiguredSqlInput, ConstraintEnforcement, ConstraintEvidence, ConstraintProvenance,
    ConstraintSourceKind, LiteralValue, MergeAction, MergeMatchKind, PhysicalWriteRow,
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

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-154-acceptance-freeze.json")).unwrap()
}

fn classify(dialect: &str, statement: &str) -> &'static str {
    let parser = dialect_from_name(dialect).expect("advertised dialect");
    let Ok(bundle) = analyze_inputs(&[SqlInput::inline(statement)], dialect, parser.as_ref())
    else {
        return "parser_boundary";
    };
    match bundle.inputs()[0]
        .statements()
        .first()
        .expect("one statement")
    {
        ProtocolStatement::Unsupported(_) => "canonical_unsupported",
        ProtocolStatement::Query(query) => match query.write() {
            Some(write)
                if write.kind() == WriteKind::ConditionalMutation
                    && write.match_condition().is_some()
                    && !write.merge_clauses().is_empty() =>
            {
                assert_eq!(query.sources().len(), 1, "{dialect}: one independent source");
                assert_eq!(query.sources()[0].name(), "source", "{dialect}: source");
                assert_eq!(write.target(), "target", "{dialect}: target");
                assert!(
                    write.merge_clauses().iter().all(|branch| {
                        matches!(
                            branch.match_kind(),
                            MergeMatchKind::Matched
                                | MergeMatchKind::NotMatched
                                | MergeMatchKind::NotMatchedByTarget
                                | MergeMatchKind::NotMatchedBySource
                        ) && matches!(
                            branch.action(),
                            MergeAction::Update { .. }
                                | MergeAction::Delete
                                | MergeAction::Insert { .. }
                        )
                    }),
                    "{dialect}: unsupported action or match shape"
                );
                "parsed_canonical"
            }
            _ => "canonical_unsupported",
        },
        _ => "canonical_unsupported",
    }
}

#[test]
fn all_thirteen_dialects_match_frozen_unmatched_merge_parser_matrix() {
    let matrix = frozen();
    let mut observed = BTreeMap::new();
    for dialect in DIALECTS {
        let cases = matrix["parser_syntax"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| classify(dialect, item.as_str().unwrap()))
            .collect::<Vec<_>>();
        observed.insert(dialect.to_string(), cases);
    }
    let actual = serde_json::to_value(&observed).unwrap();
    assert_eq!(actual, matrix["dialect_expected"], "TASK-154 parser matrix drift: {actual}");
    assert_eq!(matrix["dialects"], serde_json::json!(DIALECTS));
    assert_eq!(matrix["parser_syntax"].as_array().unwrap().len(), 5);
    assert_eq!(observed.values().map(Vec::len).sum::<usize>(), 65);
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
id: task154parity
name: Task 153 equivalent MERGE evidence
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
        ConstraintProvenance::new(ConstraintSourceKind::ExternalMetadata, "task-154-parity")
            .unwrap(),
        ConstraintEnforcement::Enforced,
    )]
}

fn state(relation: &str, value: &Value) -> PhysicalWriteState {
    let rows = value
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            PhysicalWriteRow::new(BTreeMap::from([
                (
                    "id".to_string(),
                    entry["id"]
                        .as_i64()
                        .map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                ),
                (
                    "amount".to_string(),
                    entry["score"]
                        .as_i64()
                        .map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string())),
                ),
            ]))
            .unwrap()
        })
        .collect();
    PhysicalWriteState::new(relation, rows).unwrap()
}

fn case_plan(case: &Value, catalog: &RelationCatalog) -> PhysicalWriteStatePlan {
    let statement = case["sql"]
        .as_str()
        .unwrap()
        .replace("target", TARGET)
        .replace("source", SOURCE)
        .replace("score", "amount");
    let sql = SqlInput::inline(&statement);
    let parser = dialect_from_name("postgresql").unwrap();
    let input = ConfiguredSqlInput::new("task154-parity", &sql, "postgresql", parser.as_ref());
    let mut bundle = analyze_configured_inputs_with_catalog(&[input], catalog).unwrap();
    if case["pk"] == true {
        bundle.enrich_relation_constraints(&[RelationConstraintSet::new(
            TARGET,
            vec![RelationConstraint::primary_key(vec!["id".to_string()], evidence()).unwrap()],
        )
        .unwrap()]);
    }
    let mut snapshots = vec![state(TARGET, &case["target_before"])];
    if case["source"].is_array() {
        snapshots.push(state(SOURCE, &case["source"]));
    }
    physical_ordered_write_state_plan(&bundle, &snapshots)
}

#[test]
fn all_frozen_unmatched_merge_proofs_are_equivalent_across_adapter_metadata() {
    let matrix = frozen();
    assert_eq!(
        matrix["adapter_sources"],
        serde_json::json!([
            "direct SQL catalog",
            "dbt manifest/catalog equivalent schema",
            "ODCS v3.2 equivalent schema"
        ])
    );
    assert_eq!(matrix["cases"].as_array().unwrap().len(), 12);
    for case in matrix["cases"].as_array().unwrap() {
        let direct = case_plan(case, &direct_catalog());
        let dbt = case_plan(case, &dbt_catalog());
        assert_eq!(direct, dbt, "{}: dbt catalog parity", case["id"]);
        #[cfg(feature = "odcs")]
        assert_eq!(
            direct,
            case_plan(case, &odcs_catalog()),
            "{}: ODCS schema parity",
            case["id"]
        );
        match case["proof"].as_str().unwrap() {
            "Exact" => {
                let PhysicalWriteStatePlan::Feasible {
                    transitions,
                    final_states,
                } = &direct
                else {
                    panic!("{}: missing Exact proof: {direct:?}", case["id"]);
                };
                assert_eq!(transitions.len(), 1);
                assert_eq!(
                    transitions[0].after(),
                    &state(TARGET, &case["target_after"])
                );
                assert_eq!(
                    final_states.iter().find(|v| v.relation() == SOURCE),
                    Some(&state(SOURCE, &case["source"]))
                );
            }
            "Impossible" => assert!(
                matches!(direct, PhysicalWriteStatePlan::Impossible { .. }),
                "{}: {direct:?}",
                case["id"]
            ),
            "Residual" => assert!(
                matches!(direct, PhysicalWriteStatePlan::Residual { .. }),
                "{}: {direct:?}",
                case["id"]
            ),
            other => panic!("unexpected frozen proof: {other}"),
        }
    }
}
