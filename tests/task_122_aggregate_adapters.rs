//! TASK-122 canonical DISTINCT, FILTER and HAVING parity across SQL, dbt and ODCS.
#[cfg(feature = "odcs")]
#[test]
fn distinct_filter_having_groups_are_source_independent() {
    use std::collections::BTreeMap;

    use serde_json::{json, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_aggregate_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteAggregateProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };

    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-122-acceptance-freeze.json"))
            .expect("task-owned acceptance matrix");
    let relation = "warehouse.raw.s";
    let schema = RelationSchema::new(
        relation,
        ["id", "k", "x"]
            .into_iter()
            .map(|name| {
                SchemaColumn::from_sql_type(
                    name,
                    if name == "id" { "VARCHAR" } else { "INTEGER" },
                    "postgresql",
                )
                .expect("typed source")
            })
            .collect(),
    )
    .expect("source schema");
    let catalog = RelationCatalog::from_schemas(&[schema]).expect("direct catalog");
    let known = RelationCatalog::new(&[relation]).expect("known physical source");
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task122aggregates
name: Typed grouped HAVING input
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
    .expect("ODCS schema");
    let odcs_catalog =
        RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS canonical schema");

    let parser = dialect_from_name("postgresql").expect("postgresql");
    for case in frozen["cases"].as_array().expect("task-owned cases") {
        let id = case["id"].as_str().expect("fixture");
        let sql = case["sql"]
            .as_str()
            .expect("SQL")
            .replace("FROM s", "FROM warehouse.raw.s");
        let source = SqlInput::inline(&sql);
        let inputs = [ConfiguredSqlInput::new(
            id,
            &source,
            "postgresql",
            parser.as_ref(),
        )];
        let direct = analyze_configured_inputs_with_catalog(&inputs, &catalog)
            .expect("direct SQL with typed source");
        let external = analyze_configured_inputs_with_catalog(&inputs, &odcs_catalog)
            .expect("ODCS typed SQL source");
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8",
                "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task122": {
                    "unique_id": "model.demo.task122",
                    "resource_type": "model",
                    "relation_name": "warehouse.analytics.task122",
                    "database": "warehouse",
                    "schema": "analytics",
                    "original_file_path": "models/task122.sql",
                    "language": "sql",
                    "raw_code": sql,
                    "compiled_code": sql,
                    "depends_on": {"nodes": ["source.demo.s"]}
                }
            },
            "sources": {
                "source.demo.s": {
                    "unique_id": "source.demo.s",
                    "resource_type": "source",
                    "relation_name": "warehouse.raw.s",
                    "database": "warehouse",
                    "schema": "raw",
                    "name": "s",
                    "columns": {
                        "id": {"name":"id", "data_type":"VARCHAR"},
                        "k": {"name":"k", "data_type":"INTEGER"},
                        "x": {"name":"x", "data_type":"INTEGER"}
                    }
                }
            }
        });
        let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled dbt model");
        let dbt = analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref())
            .expect("dbt analysis with schema evidence");

        let state = PhysicalWriteState::new(
            relation,
            case["source_bag"]
                .as_array()
                .expect("closed-world rows")
                .iter()
                .map(|row| {
                    PhysicalWriteRow::new(
                        row.as_object()
                            .expect("source row")
                            .iter()
                            .map(|(name, value)| {
                                let scalar = if value.is_null() {
                                    LiteralValue::Null
                                } else if let Some(number) = value.as_i64() {
                                    LiteralValue::Number(number.to_string())
                                } else {
                                    LiteralValue::Text(
                                        value.as_str().expect("row identity").to_string(),
                                    )
                                };
                                (name.clone(), scalar)
                            })
                            .collect::<BTreeMap<_, _>>(),
                    )
                    .expect("complete physical row")
                })
                .collect(),
        )
        .expect("physical source relation");

        let mut results = Vec::new();
        for (adapter, bundle) in [("SQL", &direct), ("dbt", &dbt), ("ODCS", &external)] {
            let layer = bundle
                .layers()
                .iter()
                .find(|layer| layer.consumes().iter().any(|input| input == relation))
                .unwrap_or_else(|| panic!("{adapter}:{id}: source not in canonical graph"));
            let proof = physical_finite_aggregate_bag_plan(
                bundle,
                layer.id(),
                std::slice::from_ref(&state),
            );
            let PhysicalFiniteAggregateProof::Exact(exact) = proof else {
                panic!("{adapter}:{id}: missing exact same-source aggregate law: {proof:?}");
            };
            assert_eq!(exact.source, relation, "{adapter}:{id}: physical source");
            results.push((
                exact.terminal.clone(),
                exact.terminal_domains.clone(),
                exact.groups.clone(),
            ));
        }
        assert_eq!(results[0], results[1], "{id}: compiled dbt parity");
        assert_eq!(results[0], results[2], "{id}: ODCS schema parity");
    }
}
