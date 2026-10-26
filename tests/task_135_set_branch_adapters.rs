//! TASK-135: complete set-branch page bags from direct SQL, dbt and ODCS.
#![cfg(feature = "odcs")]

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, dialect_from_name,
    parse_dbt_manifest, parse_odcs_yaml, physical_finite_set_tree_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalFiniteSetTreeProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const RELATION: &str = "warehouse.raw.s";

fn schema() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        RELATION,
        vec![
            SchemaColumn::from_sql_type("id", "TEXT", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap()])
    .unwrap()
}

#[test]
fn bounded_set_branches_have_equal_source_and_terminal_bags_on_all_adapters() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-135-acceptance-freeze.json")).unwrap();
    let known = RelationCatalog::new(&[RELATION]).unwrap();
    let contract = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task135set
name: Bounded set source
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: id
        physicalType: TEXT
      - name: k
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    let contract_schema = RelationCatalog::from_schemas(contract.source_schemas()).unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();
    let cases = freeze["additional_mandatory_cases"].as_array().unwrap();
    let mut verified = 0;
    for case in cases
        .iter()
        .filter(|c| c["id"] == "76-C135-P-set-branch" || c["id"] == "76-C135-N-set-zero")
    {
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s", "FROM warehouse.raw.s");
        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "TASK-135-set",
            &input,
            "postgresql",
            dialect.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &schema()).unwrap();
        let odcs = analyze_configured_inputs_with_catalog(&configured, &contract_schema).unwrap();
        let manifest_json = json!({
            "metadata": {"dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json", "dbt_version": "1.11.8", "adapter_type": "postgres"},
            "nodes": {"model.demo.task135set": {
                "unique_id": "model.demo.task135set", "resource_type": "model",
                "relation_name": "warehouse.analytics.task135set",
                "database": "warehouse", "schema": "analytics",
                "original_file_path": "models/task135set.sql", "language": "sql",
                "raw_code": sql, "compiled_code": sql,
                "depends_on": {"nodes": ["source.demo.s"]}
            }},
            "sources": {"source.demo.s": {
                "unique_id": "source.demo.s", "resource_type": "source",
                "relation_name": RELATION,
                "database": "warehouse", "schema": "raw", "name": "s",
                "columns": {"id": {"name": "id", "data_type": "TEXT"}, "k": {"name": "k", "data_type": "INTEGER"}}
            }}
        });
        let manifest = parse_dbt_manifest(&manifest_json.to_string()).unwrap();
        let dbt =
            analyze_dbt_manifest_with_schemas(&manifest, "postgresql", dialect.as_ref()).unwrap();
        let state = PhysicalWriteState::new(
            RELATION,
            case["source_bag"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    PhysicalWriteRow::new(BTreeMap::from([
                        ("id".to_string(), LiteralValue::Text(format!("row{i}"))),
                        (
                            "k".to_string(),
                            k.as_i64().map_or(LiteralValue::Null, |v| {
                                LiteralValue::Number(v.to_string())
                            }),
                        ),
                    ]))
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let mut proofs = Vec::new();
        for (name, bundle) in [("sql", direct), ("dbt", dbt), ("odcs", odcs)] {
            let target = bundle.layers().last().unwrap().id();
            let proof =
                physical_finite_set_tree_plan(&bundle, target, std::slice::from_ref(&state));
            let PhysicalFiniteSetTreeProof::Exact(exact) = proof else {
                panic!("{name}: {} must be exact: {proof:?}", case["id"]);
            };
            let expected = case["terminal_bag"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    BTreeMap::from([(
                        "k".to_string(),
                        v.as_i64()
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    )])
                })
                .collect::<Vec<_>>();
            assert_eq!(exact.terminal, expected, "{name}");
            assert_eq!(exact.sources, [RELATION], "{name}");
            let first = &exact.branches[0];
            let second = &exact.branches[1];
            assert_eq!(
                first.selected_row_ids,
                case["left_selected"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<_>>(),
                "{name}"
            );
            assert_eq!(
                first.page_rejected_row_ids,
                case["left_page_rejected"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<_>>(),
                "{name}"
            );
            assert_eq!(
                second.selected_row_ids,
                case["right_selected"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<_>>(),
                "{name}"
            );
            proofs.push((
                exact.sources,
                exact.branches,
                exact.tuples,
                exact.terminal,
                exact.terminal_domains,
            ));
            verified += 1;
        }
        assert_eq!(proofs[0], proofs[1], "SQL/dbt canonical parity");
        assert_eq!(proofs[0], proofs[2], "SQL/ODCS canonical parity");
    }
    assert_eq!(verified, 6);
}
