//! TASK-135: direct SQL, compiled dbt and ODCS canonical proof parity.
#![cfg(feature = "odcs")]

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, dialect_from_name,
    parse_dbt_manifest, parse_odcs_yaml, physical_finite_composed_page_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalComposedPageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const RELATION: &str = "warehouse.raw.s";

fn source(values: &[Value]) -> PhysicalWriteState {
    PhysicalWriteState::new(
        RELATION,
        values
            .iter()
            .map(|value| {
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    value
                        .as_i64()
                        .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                )]))
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn cte_pagination_retains_exact_source_identity_and_terminal_domains_across_three_adapters() {
    let inventory: Value =
        serde_json::from_str(include_str!("../docs/task-135-acceptance-freeze.json")).unwrap();
    let known = RelationCatalog::new(&[RELATION]).unwrap();
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task135pages
name: Source for bounded page
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: k
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .unwrap();
    let contract_schema = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let direct_schema = RelationCatalog::from_schemas(&[RelationSchema::new(
        RELATION,
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap();
    let dialect = dialect_from_name("postgresql").unwrap();

    let mut checked = 0;
    let cases = inventory["cases"].as_array().unwrap().iter().chain(
        inventory["additional_mandatory_cases"].as_array().unwrap().iter()
            .filter(|case| case["id"] == "76-C135-P-renamed-producer")
    );
    for case in cases {
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s", "FROM warehouse.raw.s");
        let input = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            "TASK-135",
            &input,
            "postgresql",
            dialect.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &direct_schema).unwrap();
        let odcs = analyze_configured_inputs_with_catalog(&configured, &contract_schema).unwrap();
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8", "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task135": {
                    "unique_id": "model.demo.task135", "resource_type": "model",
                    "relation_name": "warehouse.analytics.task135",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task135.sql", "language": "sql",
                    "raw_code": sql, "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.s"]}
                }
            },
            "sources": {
                "source.demo.s": {
                    "unique_id": "source.demo.s", "resource_type": "source",
                    "relation_name": RELATION,
                    "database": "warehouse", "schema": "raw", "name": "s",
                    "columns": {"k": {"name":"k", "data_type":"INTEGER"}}
                }
            }
        });
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt =
            analyze_dbt_manifest_with_schemas(&manifest, "postgresql", dialect.as_ref()).unwrap();

        let state = source(case["source_bag"].as_array().unwrap());
        let mut proofs = Vec::new();
        for (adapter, bundle) in [
            ("direct_sql", direct),
            ("dbt_compiled", dbt),
            ("odcs", odcs),
        ] {
            let target = bundle.layers().last().unwrap().id();
            let result =
                physical_finite_composed_page_plan(&bundle, target, std::slice::from_ref(&state));
            let PhysicalComposedPageProof::Exact(exact) = result else {
                panic!("{adapter}: {}: expected exact page: {result:?}", case["id"]);
            };
            assert_eq!(exact.source, RELATION, "{adapter}");
            let expected_rows = case["terminal_bag"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| {
                    BTreeMap::from([(
                        "k".to_string(),
                        value
                            .as_i64()
                            .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    )])
                })
                .collect::<Vec<_>>();
            assert_eq!(exact.terminal, expected_rows, "{adapter}");
            assert_eq!(
                exact.selected_source_ids,
                case.get("selected_source_ids")
                    .unwrap_or(&case["selected_ids"])
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap().replace("s#", "warehouse.raw.s#"))
                    .collect::<Vec<_>>(),
                "{adapter}"
            );
            proofs.push((exact.source_row_ids, exact.terminal, exact.terminal_domains));
            checked += 1;
        }
        assert_eq!(proofs[0], proofs[1], "direct/dbt parity");
        assert_eq!(proofs[0], proofs[2], "direct/ODCS parity");
    }
    assert_eq!(checked, 9);
}
