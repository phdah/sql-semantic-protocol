//! TASK-128: direct SQL, compiled dbt and ODCS integer-window parity on complete bags.

#[cfg(feature = "odcs")]
#[test]
fn advanced_windows_agree_across_direct_compiled_dbt_and_odcs_on_complete_bags() {
    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_window_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };
    use std::collections::BTreeMap;

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-128-acceptance-freeze.json")).unwrap();
    let source_name = "warehouse.raw.s";
    let parser = dialect_from_name("postgresql").unwrap();
    let sql_dialect = "postgresql";
    let external_names = RelationCatalog::new(&[source_name]).unwrap();
    for case in freeze["cases"].as_array().unwrap() {
        let x_type = "INTEGER";
        let contract = format!(
            r#"
apiVersion: v3.2.0
kind: DataContract
id: task128windows
name: Integer window source
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: id
        physicalType: INTEGER
      - name: k
        physicalType: INTEGER
      - name: x
        physicalType: {x_type}
"#
        );
        let odcs = parse_odcs_yaml(&contract, sql_dialect, &external_names).unwrap();
        let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s", "FROM warehouse.raw.s");
        let input = SqlInput::inline(&sql);
        let source_schema = RelationSchema::new(
            source_name,
            [("id", "INTEGER"), ("k", "INTEGER"), ("x", x_type)]
                .into_iter()
                .map(|(name, ty)| SchemaColumn::from_sql_type(name, ty, sql_dialect).unwrap())
                .collect(),
        )
        .unwrap();
        let direct_catalog = RelationCatalog::from_schemas(&[source_schema]).unwrap();
        let direct = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                id,
                &input,
                sql_dialect,
                parser.as_ref(),
            )],
            &direct_catalog,
        )
        .unwrap();
        let external = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                id,
                &input,
                sql_dialect,
                parser.as_ref(),
            )],
            &odcs_catalog,
        )
        .unwrap();
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8", "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task128_window": {
                    "unique_id": "model.demo.task128_window", "resource_type": "model",
                    "relation_name": "warehouse.analytics.task128_window",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task128_window.sql",
                    "language": "sql", "raw_code": sql, "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.s"]}
                }
            },
            "sources": {
                "source.demo.s": {
                    "unique_id": "source.demo.s", "resource_type": "source",
                    "relation_name": source_name, "database": "warehouse",
                    "schema": "raw", "name": "s",
                    "columns": {
                        "id": {"name":"id", "data_type":"INTEGER"},
                        "k": {"name":"k", "data_type":"INTEGER"},
                        "x": {"name":"x", "data_type":x_type}
                    }
                }
            }
        });
        let dbt_manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt =
            analyze_dbt_manifest_with_schemas(&dbt_manifest, sql_dialect, parser.as_ref()).unwrap();
        let physical = PhysicalWriteState::new(
            source_name,
            case["source_bag"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    PhysicalWriteRow::new(
                        row.as_object()
                            .unwrap()
                            .iter()
                            .map(|(name, val)| {
                                let literal = if val.is_null() {
                                    LiteralValue::Null
                                } else {
                                    LiteralValue::Number(val.to_string())
                                };
                                (name.clone(), literal)
                            })
                            .collect::<BTreeMap<_, _>>(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let mut results = Vec::new();
        for (adapter, bundle) in [
            ("direct_sql", direct),
            ("dbt_compiled", dbt),
            ("odcs_schema", external),
        ] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| layer.consumes().iter().any(|input| input == source_name))
                .unwrap_or_else(|| panic!("{id}/{adapter}: no source-backed layer"));
            let proof = physical_finite_window_bag_plan(
                &bundle,
                layer.id(),
                std::slice::from_ref(&physical),
            );
            match case["proof"].as_str().unwrap() {
                "Exact" => {
                    assert_eq!(case["classification"], "exact_required", "{id}/{adapter}");
                    let PhysicalFiniteWindowProof::Exact(exact) = proof else {
                        panic!(
                            "{id}/{adapter}: adapter lost integer window physical proof: {proof:?}"
                        );
                    };
                    assert_eq!(exact.source, source_name);
                    results.push(format!(
                        "{:?}",
                        (
                            exact.source_row_ids,
                            exact.terminal_row_ids,
                            exact.rejected_row_ids,
                            exact.terminal,
                            exact.terminal_domains
                        )
                    ));
                }
                "Residual" => {
                    assert_eq!(case["classification"], "conditional", "{id}/{adapter}");
                    assert!(
                        matches!(proof, PhysicalFiniteWindowProof::Residual { .. }),
                        "{id}/{adapter}: unattested window law must stay Residual: {proof:?}"
                    );
                    results.push("Residual".into());
                }
                other => panic!("{id}/{adapter}: unreviewed numeric classification: {other}"),
            }
        }
        assert_eq!(results.len(), 3, "{id}");
        assert_eq!(
            results[0], results[1],
            "{id}: compiled dbt advanced-window parity"
        );
        assert_eq!(results[0], results[2], "{id}: ODCS advanced-window parity");
    }
}
