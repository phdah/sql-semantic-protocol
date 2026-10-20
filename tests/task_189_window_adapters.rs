//! TASK-189: direct SQL, compiled dbt SQL and ODCS evidence converge on
//! the same closed-world navigation and ROWS-frame source and terminal outcomes.

#[cfg(feature = "odcs")]
#[test]
fn direct_dbt_and_odcs_preserve_exact_complete_window_aggregate_bags() {
    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_window_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteWindowProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };
    use sqlparser::parser::Parser;
    use std::collections::BTreeMap;

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-189-acceptance-freeze.json"))
            .expect("frozen TASK-189 matrix");
    let name = "warehouse.raw.s";
    let dialect = "postgresql";
    let parser = dialect_from_name(dialect).unwrap();

    for case in freeze["cases"].as_array().unwrap() {
        // Conditional DECIMAL evidence needs its real catalog type; otherwise
        // invalid source values would mask an unproved semantic law.
        let x_type = case["source_schema"]["x"].as_str().unwrap_or("INTEGER");
        let schema = RelationSchema::new(
            name,
            [("id", "INTEGER"), ("k", "INTEGER"), ("x", x_type)]
                .into_iter()
                .map(|(column, kind)| SchemaColumn::from_sql_type(column, kind, dialect).unwrap())
                .collect(),
        )
        .unwrap();
        let direct_catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
        let known = RelationCatalog::new(&[name]).unwrap();
        let contract = format!(
            r#"
apiVersion: v3.2.0
kind: DataContract
id: task189window
name: Window aggregate canonical source
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
        let odcs = parse_odcs_yaml(&contract, dialect, &known)
            .expect("conditional source schema evidence");
        let external_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s", "FROM warehouse.raw.s");
        let parsed = Parser::parse_sql(parser.as_ref(), &sql);
        match case["parser_status_by_dialect"][dialect].as_str() {
            Some("parsed") => {
                assert!(
                    parsed.is_ok(),
                    "{id}/{dialect}: frozen parser cell failed: {parsed:?}"
                );
            }
            Some("parser_boundary") if case["proof"] != "Exact" => {
                assert!(parsed.is_err(), "{id}/{dialect}: parser boundary changed");
                continue;
            }
            disposition => panic!("{id}/{dialect}: invalid parser cell {disposition:?}"),
        }
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
        .expect("direct canonical SQL");
        let external = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                id,
                &input,
                dialect,
                parser.as_ref(),
            )],
            &external_catalog,
        )
        .expect("ODCS canonical SQL");
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8", "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task189": {
                    "unique_id": "model.demo.task189",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task189",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task189.sql",
                    "language": "sql", "raw_code": sql, "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.s"]}
                }
            },
            "sources": {
                "source.demo.s": {
                    "unique_id": "source.demo.s", "resource_type": "source",
                    "relation_name": name, "database": "warehouse",
                    "schema": "raw", "name": "s",
                    "columns": {
                        "id": {"name":"id", "data_type":"INTEGER"},
                        "k": {"name":"k", "data_type":"INTEGER"},
                        "x": {"name":"x", "data_type":"INTEGER"}
                    }
                }
            }
        });
        let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let dbt = analyze_dbt_manifest_with_schemas(&manifest, dialect, parser.as_ref())
            .expect("compiled dbt canonical SQL");
        let state = PhysicalWriteState::new(
            name,
            case["source_bag"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    PhysicalWriteRow::new(
                        row.as_object()
                            .unwrap()
                            .iter()
                            .map(|(column, cell)| {
                                let value = if cell.is_null() {
                                    LiteralValue::Null
                                } else if cell.is_number() {
                                    LiteralValue::Number(cell.to_string())
                                } else {
                                    LiteralValue::Text(cell.as_str().unwrap().to_string())
                                };
                                (column.clone(), value)
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
            ("odcs_schema", external),
        ] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| layer.consumes().iter().any(|input| input == name))
                .unwrap_or_else(|| panic!("{id}/{adapter}: no canonical source layer"));
            let proof =
                physical_finite_window_bag_plan(&bundle, layer.id(), std::slice::from_ref(&state));
            if case["proof"] == "Exact" {
                assert_eq!(case["expected_adapter_cells"][adapter], "Exact_required");
                let PhysicalFiniteWindowProof::Exact(exact) = proof else {
                    panic!("{id}/{adapter}: window semantics lost through adapter: {proof:?}");
                };
                assert_eq!(exact.source, name);
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
            } else {
                assert_eq!(case["expected_adapter_cells"][adapter], "Residual_required");
                assert!(
                    matches!(proof, PhysicalFiniteWindowProof::Residual { .. }),
                    "{id}/{adapter}: unsupported runtime law must not be claimed: {proof:?}"
                );
            }
        }
        if case["proof"] == "Exact" {
            assert_eq!(outcomes.len(), 3, "{id}: all adapter cells executed");
            assert_eq!(outcomes[0], outcomes[1], "{id}: compiled dbt parity");
            assert_eq!(outcomes[0], outcomes[2], "{id}: ODCS parity");
        }
    }
}
