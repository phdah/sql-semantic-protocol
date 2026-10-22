//! TASK-129 adapter parity: SQL, compiled dbt and ODCS source evidence must
//! produce the same complete canonical EXISTS/NOT EXISTS physical proof.

#[cfg(feature = "odcs")]
#[test]
fn frozen_exists_bags_are_identical_for_direct_dbt_and_odcs_sources() {
    use std::collections::BTreeMap;

    use serde_json::{json, Map, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml, physical_finite_exists_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteExistsProof, PhysicalWriteRow,
        PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
    };

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-129-acceptance-freeze.json"))
            .expect("frozen TASK-129 matrix");
    let parser = dialect_from_name("postgresql").expect("postgres parser");
    let source_names = ["warehouse.raw.outer_rows", "warehouse.raw.inner_rows"];
    let named = RelationCatalog::new(&source_names).expect("ODCS source names");
    let odcs = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task129exists
name: Physical EXISTS source contracts
version: 1.0.0
status: active
schema:
  - name: outer_rows
    physicalName: warehouse.raw.outer_rows
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
  - name: inner_rows
    physicalName: warehouse.raw.inner_rows
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
"#,
        "postgresql",
        &named,
    )
    .expect("ODCS schema");
    let external = RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS catalog");
    let schemas = source_names
        .iter()
        .map(|name| {
            RelationSchema::new(
                *name,
                vec![
                    SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").expect("identity"),
                    SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").expect("key"),
                ],
            )
            .expect("source")
        })
        .collect::<Vec<_>>();
    let direct_catalog = RelationCatalog::from_schemas(&schemas).expect("direct catalog");

    for case in freeze["cases"].as_array().expect("complete cases") {
        let id = case["id"].as_str().expect("fixture ID");
        let sql = case["sql"]
            .as_str()
            .expect("frozen SQL")
            .replace("FROM outer_rows o", "FROM warehouse.raw.outer_rows AS o")
            .replace("FROM inner_rows i", "FROM warehouse.raw.inner_rows AS i");
        let input = SqlInput::inline(&sql);
        let direct = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(id, &input, "postgresql", parser.as_ref())],
            &direct_catalog,
        )
        .expect("direct SQL");
        let from_odcs = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(id, &input, "postgresql", parser.as_ref())],
            &external,
        )
        .expect("ODCS-derived physical schema");

        let sources = ["outer_rows", "inner_rows"]
            .into_iter()
            .map(|relation| {
                let mut columns = Map::new();
                for (column, ty) in [("id", "VARCHAR"), ("k", "INTEGER")] {
                    columns.insert(
                        column.to_string(),
                        json!({"name":column,"data_type":ty}),
                    );
                }
                (
                    format!("source.demo.{relation}"),
                    json!({
                        "unique_id":format!("source.demo.{relation}"),
                        "resource_type":"source",
                        "relation_name":format!("warehouse.raw.{relation}"),
                        "database":"warehouse","schema":"raw",
                        "name":relation,"columns":columns,
                    }),
                )
            })
            .collect::<Map<_, _>>();
        let manifest = json!({
            "metadata":{
                "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version":"1.11.8","adapter_type":"postgres"
            },
            "nodes":{
                "model.demo.task129":{
                    "unique_id":"model.demo.task129",
                    "resource_type":"model",
                    "relation_name":"warehouse.analytics.task129",
                    "database":"warehouse","schema":"analytics",
                    "original_file_path":"models/task129.sql",
                    "language":"sql","raw_code":sql,"compiled_code":sql,
                    "depends_on":{"nodes":["source.demo.outer_rows","source.demo.inner_rows"]}
                }
            },
            "sources":sources
        });
        let parsed = parse_dbt_manifest(&manifest.to_string()).expect("valid compiled manifest");
        let dbt =
            analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref())
                .expect("dbt authoritative sources");

        let states = ["outer_rows", "inner_rows"]
            .into_iter()
            .map(|relation| {
                let physical = case["source_bags"][relation]
                    .as_array()
                    .expect("complete physical rows")
                    .iter()
                    .map(|row| {
                        PhysicalWriteRow::new(BTreeMap::from([
                            (
                                "id".to_string(),
                                LiteralValue::Text(
                                    row["id"].as_str().expect("stable id").to_string(),
                                ),
                            ),
                            (
                                "k".to_string(),
                                row["k"].as_i64().map_or(LiteralValue::Null, |v| {
                                    LiteralValue::Number(v.to_string())
                                }),
                            ),
                        ]))
                        .expect("typed complete row")
                    })
                    .collect();
                PhysicalWriteState::new(format!("warehouse.raw.{relation}"), physical)
                    .expect("physical source")
            })
            .collect::<Vec<_>>();

        let mut results = Vec::new();
        for (adapter, bundle) in [
            ("direct_sql", direct),
            ("dbt_compiled", dbt),
            ("odcs_schema", from_odcs),
        ] {
            let target = bundle
                .layers()
                .iter()
                .find(|layer| layer.consumes().iter().any(|relation|
                    relation == source_names[0] || relation == source_names[1]
                ))
                .unwrap_or_else(|| panic!("{id}/{adapter}: lost physical dependencies"));
            let proof = physical_finite_exists_bag_plan(&bundle, target.id(), &states);
            let PhysicalFiniteExistsProof::Exact(exact) = proof else {
                panic!("{id}/{adapter}: lost canonical physical evidence: {proof:?}");
            };
            assert_eq!(exact.sources, source_names.map(str::to_string));
            results.push(format!(
                "{:?}",
                (
                    exact.kind,
                    exact.selected,
                    exact.rejected,
                    exact.matching_inner,
                    exact.terminal,
                    exact.terminal_domains,
                )
            ));
        }
        assert_eq!(results[0], results[1], "{id}: dbt canonical parity");
        assert_eq!(results[0], results[2], "{id}: ODCS canonical parity");
    }
}
