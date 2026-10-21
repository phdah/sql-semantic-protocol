//! TASK-127: the frozen direct-source QUALIFY oracle must survive each metadata adapter.
//! Joined and grouped upstream materialization retain separate mandatory gates.
#[cfg(feature = "odcs")]
#[test]
fn direct_dbt_and_odcs_preserve_qualify_survivors_and_rejections() {
    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_window_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };
    use std::collections::BTreeMap;

    let matrix: Value =
        serde_json::from_str(include_str!("../docs/task-127-acceptance-freeze.json")).unwrap();
    let source = "warehouse.raw.s";
    let dialect = "postgresql";
    let parser = dialect_from_name(dialect).unwrap();
    let named = RelationCatalog::new(&[source]).unwrap();
    let contract = r#"
apiVersion: v3.2.0
kind: DataContract
id: task127qualify
name: QUALIFY portable physical schema
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
        physicalType: INTEGER
"#;
    let odcs = parse_odcs_yaml(contract, dialect, &named).unwrap();
    let external = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let schema = RelationSchema::new(
        source,
        ["id", "k", "x"]
            .into_iter()
            .map(|name| SchemaColumn::from_sql_type(name, "INTEGER", dialect).unwrap())
            .collect(),
    )
    .unwrap();
    let direct_catalog = RelationCatalog::from_schemas(&[schema]).unwrap();

    let mut visited = 0;
    for case in matrix["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if !matches!(
            id,
            "74-C127-P1" | "74-C127-P2" | "74-C127-P3" | "74-C127-P6" | "74-C127-N1" | "74-C127-N2"
        ) {
            continue;
        }
        visited += 1;
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s", "FROM warehouse.raw.s");
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
        let from_odcs = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                id,
                &input,
                dialect,
                parser.as_ref(),
            )],
            &external,
        )
        .unwrap();
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8", "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task127": {
                    "unique_id": "model.demo.task127",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task127",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task127.sql",
                    "language": "sql", "raw_code": sql, "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.s"]}
                }
            },
            "sources": {
                "source.demo.s": {
                    "unique_id": "source.demo.s", "resource_type": "source",
                    "relation_name": source, "database": "warehouse",
                    "schema": "raw", "name": "s",
                    "columns": {
                        "id": {"name": "id", "data_type": "INTEGER"},
                        "k": {"name": "k", "data_type": "INTEGER"},
                        "x": {"name": "x", "data_type": "INTEGER"}
                    }
                }
            }
        });
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt = analyze_dbt_manifest_with_schemas(&manifest, dialect, parser.as_ref()).unwrap();
        let state = PhysicalWriteState::new(
            source,
            case["physical_source_bags"]["s"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    PhysicalWriteRow::new(
                        row.as_object()
                            .unwrap()
                            .iter()
                            .map(|(name, value)| {
                                let literal = if value.is_null() {
                                    LiteralValue::Null
                                } else {
                                    LiteralValue::Number(value.to_string())
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
        let mut outcomes = Vec::new();
        for (adapter, bundle) in [
            ("direct_sql", direct),
            ("dbt_compiled", dbt),
            ("odcs_schema", from_odcs),
        ] {
            let target = bundle
                .layers()
                .iter()
                .find(|layer| layer.consumes().iter().any(|name| name == source))
                .unwrap_or_else(|| panic!("{id}/{adapter}: lost physical source"));
            let proof =
                physical_finite_window_bag_plan(&bundle, target.id(), std::slice::from_ref(&state));
            if id == "74-C127-N2" {
                assert!(
                    matches!(proof, PhysicalFiniteWindowProof::Residual { .. }),
                    "{id}/{adapter}: ambiguous peers must fail closed: {proof:?}"
                );
            } else {
                let PhysicalFiniteWindowProof::Exact(exact) = proof else {
                    panic!("{id}/{adapter}: lost exact QUALIFY membership: {proof:?}");
                };
                assert_eq!(exact.source, source);
                let selected = case["selected_source_row_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().replace("s#", "warehouse.raw.s#"))
                    .collect::<Vec<_>>();
                let rejected = case["rejected_source_row_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().replace("s#", "warehouse.raw.s#"))
                    .collect::<Vec<_>>();
                assert_eq!(exact.terminal_row_ids, selected, "{id}/{adapter}");
                assert_eq!(exact.rejected_row_ids, rejected, "{id}/{adapter}");
                outcomes.push(format!(
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
        }
        if id != "74-C127-N2" {
            assert_eq!(outcomes.len(), 3, "{id}: three independent adapters");
            assert_eq!(outcomes[0], outcomes[1], "{id}: dbt parity");
            assert_eq!(outcomes[0], outcomes[2], "{id}: ODCS parity");
        }
    }
    assert_eq!(visited, 6, "all six direct-source cases need parity");
}
