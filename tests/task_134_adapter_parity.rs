//! TASK-134: certified direct SQL, compiled dbt and ODCS source-schema parity.
#![cfg(feature = "odcs")]

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, dialect_from_name,
    parse_dbt_manifest, parse_odcs_yaml, physical_finite_page_bag_plan, ConfiguredSqlInput,
    LiteralValue, PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
    RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

#[test]
fn frozen_six_mandatory_sql_dbt_odcs_cases_have_identical_closed_world_bags() {
    const RELATION: &str = "warehouse.raw.s";
    let inventory: Value =
        serde_json::from_str(include_str!("../docs/task-134-acceptance-freeze.json")).unwrap();
    let known = RelationCatalog::new(&[RELATION]).unwrap();
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task134pages
name: Page source
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
    let external = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let direct_schema = RelationCatalog::from_schemas(&[RelationSchema::new(
        RELATION,
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
    )
    .unwrap()])
    .unwrap();

    let parser = dialect_from_name("postgresql").unwrap();
    let mut checked = 0;
    for case in inventory["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["classification"] == "mandatory")
    {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s", "FROM warehouse.raw.s");
        let inline = SqlInput::inline(&sql);
        let configured = [ConfiguredSqlInput::new(
            id,
            &inline,
            "postgresql",
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&configured, &direct_schema).unwrap();
        let odcs = analyze_configured_inputs_with_catalog(&configured, &external).unwrap();

        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8", "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task134": {
                    "unique_id": "model.demo.task134", "resource_type": "model",
                    "relation_name": "warehouse.analytics.task134",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task134.sql", "language": "sql",
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
        let dbt = analyze_dbt_manifest_with_schemas(&manifest, "postgresql", parser.as_ref()).unwrap();
        let source = PhysicalWriteState::new(
            RELATION,
            case["source_bag"].as_array().unwrap().iter().map(|value| {
                PhysicalWriteRow::new(BTreeMap::from([(
                    "k".to_string(),
                    value.as_i64().map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                )])).unwrap()
            }).collect(),
        ).unwrap();
        let mut proofs = Vec::new();
        for (adapter, bundle) in [("direct_sql", direct), ("dbt_compiled", dbt), ("odcs", odcs)] {
            assert_eq!(case["adapter_cells"][adapter], "exact required");
            let target = bundle.layers().iter().find(|layer|
                layer.consumes().iter().any(|dependency| dependency == RELATION)
            ).unwrap_or_else(|| panic!("{id}/{adapter}: missing source-bearing model"));
            let proof = physical_finite_page_bag_plan(&bundle, target.id(), std::slice::from_ref(&source));
            let PhysicalFinitePageProof::Exact(exact) = proof else {
                panic!("{id}/{adapter}: missing exact physical bag: {proof:?}");
            };
            assert_eq!(exact.source, RELATION);
            let mut terminal = exact
                .terminal
                .iter()
                .map(|row| format!("{:?}", row.get("k")))
                .collect::<Vec<_>>();
            terminal.sort();
            let mut expected = case["terminal_bag"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    format!(
                        "{:?}",
                        Some(
                            v.as_i64()
                                .map_or(LiteralValue::Null, |n| LiteralValue::Number(
                                    n.to_string()
                                ))
                        )
                    )
                })
                .collect::<Vec<_>>();
            expected.sort();
            assert_eq!(terminal, expected, "{id}/{adapter}: full terminal bag");
            proofs.push(format!(
                "{:?}",
                (
                    exact.rows,
                    exact.terminal,
                    exact.terminal_domains,
                    exact.terminal_contributors
                )
            ));
            checked += 1;
        }
        assert_eq!(proofs[0], proofs[1], "{id}: SQL/dbt canonical parity");
        assert_eq!(proofs[0], proofs[2], "{id}: SQL/ODCS canonical parity");
    }
    assert_eq!(
        checked, 18,
        "six mandatory cases and three evidence adapters"
    );
}
