//! TASK-132: direct, compiled dbt and ODCS exact physical proof parity.
//! The three adapters must preserve identical source identities and terminal bags.

#[cfg(feature = "odcs")]
#[test]
fn frozen_nested_composite_exists_retains_full_adapter_proof_parity() {
    use std::collections::BTreeMap;

    use serde_json::{json, Map, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml,
        physical_finite_exists_bag_plan, physical_finite_joined_exists_bag_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalFiniteExistsProof, PhysicalJoinedExistsProof,
        PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn,
        SqlInput,
    };
    let frozen: Value = serde_json::from_str(
        include_str!("../docs/task-132-acceptance-freeze.json"),
    ).expect("task 132 versioned matrix");
    let parser = dialect_from_name("postgresql").expect("dialect");
    let names = [
        "warehouse.raw.outer_rows",
        "warehouse.raw.inner_rows",
        "warehouse.raw.dim_rows",
    ];
    let named = RelationCatalog::new(&names).expect("ODCS catalog names");
    let contracts = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task132composed
name: Joined and grouped correlated sources
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
      - name: g
        physicalType: INTEGER
  - name: inner_rows
    physicalName: warehouse.raw.inner_rows
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: g
        physicalType: INTEGER
  - name: dim_rows
    physicalName: warehouse.raw.dim_rows
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: g
        physicalType: INTEGER
"#,
        "postgresql",
        &named,
    ).expect("source contracts");
    let odcs = RelationCatalog::from_schemas(contracts.source_schemas()).unwrap();
    let direct = RelationCatalog::from_schemas(
        &names.iter().map(|name|
            RelationSchema::new(
                *name,
                ["id","k","g"].iter().map(|col|
                    SchemaColumn::from_sql_type(
                        *col,
                        if *col == "id" { "VARCHAR" } else { "INTEGER" },
                        "postgresql",
                    ).unwrap()
                ).collect(),
            ).unwrap()
        ).collect::<Vec<_>>(),
    ).unwrap();

    let mut verified = 0;
    for case in frozen["cases"].as_array().unwrap() {
        if case["variant"] == "dbt_subquery_orders" {
            continue;
        }
        let sql = case["sql"].as_str().unwrap()
            .replace("FROM outer_rows o", "FROM warehouse.raw.outer_rows o")
            .replace("FROM inner_rows i", "FROM warehouse.raw.inner_rows i")
            .replace("JOIN dim_rows d", "JOIN warehouse.raw.dim_rows d");
        let input = SqlInput::inline(&sql);
        let configured = ConfiguredSqlInput::new(
            "task132", &input, "postgresql", parser.as_ref(),
        );
        let direct_bundle = analyze_configured_inputs_with_catalog(
            std::slice::from_ref(&configured), &direct,
        ).expect("direct SQL");
        let odcs_bundle = analyze_configured_inputs_with_catalog(
            &[configured], &odcs,
        ).expect("ODCS SQL");
        let relations = if case["variant"] == "joined_composite" {
            &names[..3]
        } else {
            &names[..2]
        };
        let mut sources = Map::new();
        for (relation, physical) in ["outer_rows","inner_rows","dim_rows"]
            .into_iter().zip(names)
        {
            let columns = ["id","k","g"].into_iter().map(|col| (
                col.to_string(),
                json!({
                    "name": col,
                    "data_type": if col == "id" { "VARCHAR" } else { "INTEGER" }
                }),
            )).collect::<Map<_, _>>();
            sources.insert(
                format!("source.demo.{relation}"),
                json!({
                    "unique_id":format!("source.demo.{relation}"),
                    "resource_type":"source",
                    "relation_name":physical,
                    "database":"warehouse","schema":"raw",
                    "name":relation,"columns":columns
                }),
            );
        }
        let dependencies = relations.iter().map(|physical| {
            let relation = physical.rsplit('.').next().unwrap();
            format!("source.demo.{relation}")
        }).collect::<Vec<_>>();
        let manifest = json!({
            "metadata":{
                "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version":"1.11.8","adapter_type":"postgres"
            },
            "nodes":{
                "model.demo.task132":{
                    "unique_id":"model.demo.task132",
                    "resource_type":"model",
                    "relation_name":"warehouse.analytics.task132",
                    "database":"warehouse","schema":"analytics",
                    "original_file_path":"models/task132.sql",
                    "language":"sql","raw_code":sql,"compiled_code":sql,
                    "depends_on":{"nodes":dependencies}
                }
            },
            "sources":sources
        });
        let parsed = parse_dbt_manifest(&manifest.to_string()).expect("compiled manifest");
        let dbt = analyze_dbt_manifest_with_schemas(
            &parsed, "postgresql", parser.as_ref(),
        ).expect("dbt schema evidence");

        let states = ["outer_rows","inner_rows","dim_rows"].into_iter()
            .filter(|relation| !case["source_bags"][relation].is_null())
            .map(|relation| {
                let rows = case["source_bags"][relation].as_array().unwrap().iter()
                    .map(|row| {
                        let integer = |column| row[column].as_i64().map_or(
                            LiteralValue::Null,
                            |n| LiteralValue::Number(n.to_string()),
                        );
                        PhysicalWriteRow::new(BTreeMap::from([
                            ("id".to_string(), LiteralValue::Text(row["id"].as_str().unwrap().to_string())),
                            ("k".to_string(), integer("k")),
                            ("g".to_string(), integer("g")),
                        ])).unwrap()
                    }).collect();
                PhysicalWriteState::new(format!("warehouse.raw.{relation}"), rows).unwrap()
            }).collect::<Vec<_>>();
        let mut proofs = Vec::new();
        for (adapter, bundle) in [
            ("direct SQL", direct_bundle),
            ("dbt compiled", dbt),
            ("ODCS schema", odcs_bundle),
        ] {
            let target = bundle.layers().last().expect("target model");
            let summary = if case["variant"] == "joined_composite" {
                let result = physical_finite_joined_exists_bag_plan(
                    &bundle, target.id(), &states,
                );
                let PhysicalJoinedExistsProof::Exact(proof) = result else {
                    panic!("{} {adapter} joined proof: {result:?}", case["id"]);
                };
                format!("{:?}", (
                    proof.sources, proof.selected, proof.rejected,
                    proof.matching_pairs, proof.terminal, proof.terminal_domains,
                ))
            } else {
                let result = physical_finite_exists_bag_plan(
                    &bundle, target.id(), &states,
                );
                let PhysicalFiniteExistsProof::Exact(proof) = result else {
                    panic!("{} {adapter} source proof: {result:?}", case["id"]);
                };
                format!("{:?}", (
                    proof.sources, proof.selected, proof.rejected,
                    proof.matching_inner, proof.terminal, proof.terminal_domains,
                ))
            };
            proofs.push(summary);
        }
        assert_eq!(proofs[0], proofs[1], "{} compiled dbt parity", case["id"]);
        assert_eq!(proofs[0], proofs[2], "{} ODCS parity", case["id"]);
        verified += 3;
    }
    assert_eq!(verified, 18);
}
