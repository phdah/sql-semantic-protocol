//! TASK-125: direct SQL, compiled dbt SQL and ODCS evidence converge on
//! the same canonical peer-aware physical source and terminal rank outcomes.

#[cfg(feature = "odcs")]
#[test]
fn all_three_adapters_preserve_frozen_peer_ranks_and_fail_closed_boundaries() {
    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_rank_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteRankProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };
    use sqlparser::parser::Parser;
    use std::collections::BTreeMap;

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-125-acceptance-freeze.json"))
            .expect("frozen TASK-125 matrix");
    let name = "warehouse.raw.s";
    let schema = RelationSchema::new(
        name,
        [("id", "VARCHAR"), ("k", "INTEGER"), ("x", "INTEGER")]
            .into_iter()
            .map(|(column, kind)| SchemaColumn::from_sql_type(column, kind, "postgresql").unwrap())
            .collect(),
    )
    .unwrap();
    let direct_catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let known = RelationCatalog::new(&[name]).unwrap();
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task125ranked
name: Ranking canonical source
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: x
        physicalType: INTEGER
"#,
        "postgresql",
        &known,
    )
    .expect("source schema evidence");
    let external_catalog = RelationCatalog::from_schemas(odcs.source_schemas()).unwrap();
    let dialect = "postgresql";
    let parser = dialect_from_name(dialect).unwrap();

    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM s", "FROM warehouse.raw.s");
        let parsed = Parser::parse_sql(parser.as_ref(), &sql);
        if parsed.is_err() {
            assert!(
                case["parser_condition"].is_string(),
                "{id}: mandatory SQL is not parseable: {parsed:?}"
            );
            continue;
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
                "model.demo.task125": {
                    "unique_id": "model.demo.task125",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task125",
                    "database": "warehouse", "schema": "analytics",
                    "original_file_path": "models/task125.sql",
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
                        "id": {"name":"id", "data_type":"VARCHAR"},
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
                                } else if let Some(n) = cell.as_i64() {
                                    LiteralValue::Number(n.to_string())
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
                physical_finite_rank_bag_plan(&bundle, layer.id(), std::slice::from_ref(&state));
            if case["exact"] == true {
                assert_eq!(case["expected_adapter_cells"][adapter], "Exact_required");
                let PhysicalFiniteRankProof::Exact(exact) = proof else {
                    panic!("{id}/{adapter}: rank semantics lost through adapter: {proof:?}");
                };
                assert_eq!(exact.source, name);
                outcomes.push(format!(
                    "{:?}",
                    (exact.ranked_rows, exact.terminal, exact.terminal_domains)
                ));
            } else {
                assert_eq!(case["expected_adapter_cells"][adapter], "typed_Residual");
                assert!(
                    matches!(proof, PhysicalFiniteRankProof::Residual { .. }),
                    "{id}/{adapter}: unsupported runtime law must not be claimed: {proof:?}"
                );
            }
        }
        if case["exact"] == true {
            assert_eq!(outcomes.len(), 3, "{id}: all adapter cells executed");
            assert_eq!(outcomes[0], outcomes[1], "{id}: compiled dbt parity");
            assert_eq!(outcomes[0], outcomes[2], "{id}: ODCS parity");
        }
    }
}
