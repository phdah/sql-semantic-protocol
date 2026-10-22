//! TASK-131 source evidence parity: SQL, compiled dbt and ODCS must agree
//! about exact scalar cardinality or three-valued quantified terminal rows.

#[cfg(feature = "odcs")]
#[test]
fn all_frozen_mandatory_scalar_and_quantified_cases_preserve_adapter_parity() {
    use std::collections::BTreeMap;

    use serde_json::{json, Map, Value};
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas,
        dialect_from_name, parse_dbt_manifest, parse_odcs_yaml,
        physical_finite_quantified_subquery_plan, physical_finite_scalar_subquery_plan,
        ConfiguredSqlInput, LiteralValue, PhysicalQuantifiedScalarProof,
        PhysicalScalarSubqueryProof, PhysicalWriteRow, PhysicalWriteState, RelationCatalog,
        RelationSchema, SchemaColumn, SqlInput,
    };

    let frozen: Value =
        serde_json::from_str(include_str!("../docs/task-131-acceptance-freeze.json"))
            .expect("TASK-131 frozen source matrix");
    let physical_name = "warehouse.raw.inner_rows";
    let parser = dialect_from_name("postgresql").unwrap();
    let named = RelationCatalog::new(&[physical_name]).unwrap();
    let contracts = parse_odcs_yaml(
        r#"
apiVersion: v3.2.0
kind: DataContract
id: task131
name: Scalar and quantified input source
version: 1.0.0
status: active
schema:
  - name: inner_rows
    physicalName: warehouse.raw.inner_rows
    properties:
      - name: id
        physicalType: VARCHAR
      - name: v
        physicalType: INTEGER
"#,
        "postgresql",
        &named,
    )
    .expect("ODCS contract");
    let external = RelationCatalog::from_schemas(contracts.source_schemas()).unwrap();
    let direct = RelationCatalog::from_schemas(&[RelationSchema::new(
        physical_name,
        vec![
            SchemaColumn::from_sql_type("id", "VARCHAR", "postgresql").unwrap(),
            SchemaColumn::from_sql_type("v", "INTEGER", "postgresql").unwrap(),
        ],
    )
    .unwrap()])
    .unwrap();

    let mut certified = 0;
    for case in frozen["cases"].as_array().unwrap() {
        let kind = case["kind"].as_str().unwrap();
        if !["scalar", "any", "all"].contains(&kind) {
            continue;
        }
        let sql = case["sql"]
            .as_str()
            .unwrap()
            .replace("FROM inner_rows", "FROM warehouse.raw.inner_rows");
        let input = SqlInput::inline(&sql);
        let direct_bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "task-131",
                &input,
                "postgresql",
                parser.as_ref(),
            )],
            &direct,
        )
        .unwrap();
        let odcs_bundle = analyze_configured_inputs_with_catalog(
            &[ConfiguredSqlInput::new(
                "task-131",
                &input,
                "postgresql",
                parser.as_ref(),
            )],
            &external,
        )
        .unwrap();
        let columns = Map::from_iter(
            [("id", "VARCHAR"), ("v", "INTEGER")]
                .into_iter()
                .map(|(name, ty)| (name.to_string(), json!({"name":name,"data_type":ty}))),
        );
        let manifest = json!({
            "metadata": {
                "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
                "dbt_version": "1.11.8", "adapter_type": "postgres"
            },
            "nodes": {
                "model.demo.task131": {
                    "unique_id":"model.demo.task131",
                    "resource_type":"model",
                    "relation_name":"warehouse.analytics.task131",
                    "database":"warehouse","schema":"analytics",
                    "original_file_path":"models/task131.sql",
                    "language":"sql","raw_code":sql,"compiled_code":sql,
                    "depends_on":{"nodes":["source.demo.inner_rows"]}
                }
            },
            "sources": {
                "source.demo.inner_rows": {
                    "unique_id":"source.demo.inner_rows", "resource_type":"source",
                    "relation_name":physical_name,"database":"warehouse",
                    "schema":"raw", "name":"inner_rows", "columns":columns
                }
            }
        });
        let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
        let compiled =
            analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref()).unwrap();
        let bag = PhysicalWriteState::new(
            physical_name,
            case["inner"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    PhysicalWriteRow::new(BTreeMap::from([
                        (
                            "id".to_string(),
                            LiteralValue::Text(row["id"].as_str().unwrap().to_string()),
                        ),
                        (
                            "v".to_string(),
                            row["v"].as_i64().map_or(LiteralValue::Null, |v| {
                                LiteralValue::Number(v.to_string())
                            }),
                        ),
                    ]))
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let proofs = [("direct SQL",direct_bundle),("dbt compiled",compiled),("ODCS contract",odcs_bundle)]
            .into_iter()
            .map(|(adapter,bundle)| {
                let target = bundle.layers().last().expect("one source-backed layer");
                let actual = if kind == "scalar" {
                    match physical_finite_scalar_subquery_plan(&bundle,target.id(),std::slice::from_ref(&bag)) {
                        PhysicalScalarSubqueryProof::Exact(exact) =>
                            format!("{:?}",("exact",exact.source,exact.source_row_ids,exact.value,exact.bounds)),
                        PhysicalScalarSubqueryProof::CardinalityViolation{source_row_ids} =>
                            format!("{:?}",("error",source_row_ids)),
                        other => panic!("{} {adapter}: scalar adapter lost evidence: {other:?}",case["id"]),
                    }
                } else {
                    match physical_finite_quantified_subquery_plan(&bundle,target.id(),std::slice::from_ref(&bag)) {
                        PhysicalQuantifiedScalarProof::Exact(exact) =>
                            format!("{:?}",("exact",exact.source,exact.source_row_ids,exact.truth,exact.terminal)),
                        other => panic!("{} {adapter}: quantified adapter lost evidence: {other:?}",case["id"]),
                    }
                };
                actual
            })
            .collect::<Vec<_>>();
        assert_eq!(proofs[0],proofs[1],"{} compiled dbt parity",case["id"]);
        assert_eq!(proofs[0],proofs[2],"{} ODCS parity",case["id"]);
        certified += 3;
    }
    assert_eq!(certified,36,"12 complete source cases across three adapters");
}
