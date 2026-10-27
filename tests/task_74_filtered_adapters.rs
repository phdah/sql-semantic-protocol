//! SQL, compiled dbt and ODCS source schema parity for TASK-74 filtered windows.
#[cfg(feature = "odcs")]
#[test]
fn filtered_qualify_matches_across_direct_dbt_and_odcs_adapters() {
    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_window_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };
    use std::collections::BTreeMap;

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-74-filtered-qualify-freeze.json")).unwrap();
    let dialect = "postgresql";
    let parser = dialect_from_name(dialect).unwrap();
    let relation = "warehouse.raw.s";
    let named = RelationCatalog::new(&[relation]).unwrap();
    let schema = RelationSchema::new(
        relation,
        ["id", "k", "x"]
            .into_iter()
            .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", dialect).unwrap())
            .collect(),
    )
    .unwrap();
    let direct_catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let odcs_contract = format!(
        "apiVersion: v3.2.0\nkind: DataContract\nid: task74filtered\nname: Filtered window\nversion: 1.0.0\nstatus: active\nschema:\n  - name: s\n    physicalName: {relation}\n    properties:\n      - name: id\n        physicalType: INTEGER\n      - name: k\n        physicalType: INTEGER\n      - name: x\n        physicalType: INTEGER\n"
    );
    let odcs = parse_odcs_yaml(&odcs_contract, dialect, &named).unwrap();
    let odcs_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();

    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s WHERE", "FROM warehouse.raw.s AS s WHERE");
        let input = SqlInput::inline(&sql);
        let direct = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                id,
                &input,
                dialect,
                parser.as_ref(),
            )],
            &direct_catalog,
        )
        .unwrap();
        let odcs = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                id,
                &input,
                dialect,
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
                "model.demo.task74_filtered": {
                    "unique_id": "model.demo.task74_filtered",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task74_filtered",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task74_filtered.sql",
                    "language": "sql", "raw_code": sql, "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.s"]}
                }
            },
            "sources": {
                "source.demo.s": {
                    "unique_id": "source.demo.s", "resource_type": "source",
                    "relation_name": relation,
                    "database": "warehouse", "schema": "raw", "name": "s",
                    "columns": {
                        "id": {"name":"id","data_type":"INTEGER"},
                        "k": {"name":"k","data_type":"INTEGER"},
                        "x": {"name":"x","data_type":"INTEGER"}
                    }
                }
            }
        });
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt = analyze_dbt_manifest_with_schemas(&manifest, dialect, parser.as_ref()).unwrap();
        let state = PhysicalWriteState::new(
            relation,
            case["physical_source_bags"]["s"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    PhysicalWriteRow::new(
                        row.as_object()
                            .unwrap()
                            .iter()
                            .map(|(key, value)| {
                                (
                                    key.clone(),
                                    value.as_i64().map_or(LiteralValue::Null, |number| {
                                        LiteralValue::Number(number.to_string())
                                    }),
                                )
                            })
                            .collect::<BTreeMap<_, _>>(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let mut outcomes = Vec::new();
        for (adapter, bundle) in [
            ("direct_sql", direct),
            ("dbt_compiled", dbt),
            ("odcs_schema", odcs),
        ] {
            let target = bundle
                .layers()
                .iter()
                .find(|layer| layer.consumes().iter().any(|name| name == relation))
                .unwrap_or_else(|| panic!("{id}/{adapter}: missing source layer"));
            let proof = physical_finite_window_bag_plan(&bundle, target.id(), std::slice::from_ref(&state));
            if case["proof"] == "Exact" {
                let PhysicalFiniteWindowProof::Exact(exact) = proof else {
                    panic!("{id}/{adapter}: missing Exact proof: {proof:?}");
                };
                assert_eq!(exact.physical_sources, [relation], "{id}/{adapter}");
                let actual = exact
                    .terminal
                    .iter()
                    .map(|row| format!("{row:?}"))
                    .collect::<Vec<_>>();
                let mut expected = case["expected_full_terminal_bag"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        let fields = row
                            .as_object()
                            .unwrap()
                            .iter()
                            .map(|(key, value)| {
                                (
                                    key.to_owned(),
                                    value.as_i64().map_or(LiteralValue::Null, |number| {
                                        LiteralValue::Number(number.to_string())
                                    }),
                                )
                            })
                            .collect::<BTreeMap<_, _>>();
                        format!("{fields:?}")
                    })
                    .collect::<Vec<_>>();
                expected.sort();
                let mut actual = actual;
                actual.sort();
                assert_eq!(actual, expected, "{id}/{adapter}");
                outcomes.push(format!(
                    "{:?}",
                    (
                        &exact.source_row_ids,
                        &exact.rejected_upstream_row_ids,
                        &exact.rejected_row_ids,
                        &exact.terminal_row_ids,
                        &exact.terminal,
                        &exact.terminal_domains,
                    )
                ));
            } else {
                assert!(
                    matches!(proof, PhysicalFiniteWindowProof::Residual { .. }),
                    "{id}/{adapter}: unsupported WHERE must stay Residual: {proof:?}"
                );
                outcomes.push("Residual".to_string());
            }
        }
        assert_eq!(outcomes[0], outcomes[1], "{id}: dbt parity");
        assert_eq!(outcomes[0], outcomes[2], "{id}: ODCS parity");
    }
}
