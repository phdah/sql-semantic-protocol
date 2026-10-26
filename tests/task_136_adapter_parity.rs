//! TASK-136 direct SQL, compiled dbt and ODCS evidence share canonical sampling and paging.
#![cfg(feature = "odcs")]

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, dialect_from_name,
    parse_dbt_manifest, parse_odcs_yaml, physical_finite_page_bag_plan, to_bundle_json,
    ConfiguredSqlInput, LiteralValue, PhysicalFinitePageProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const RELATION: &str = "warehouse.raw.s";

fn source() -> PhysicalWriteState {
    PhysicalWriteState::new(RELATION, [1,2,3].into_iter().map(|k| {
        PhysicalWriteRow::new(BTreeMap::from([(
            "k".to_string(), LiteralValue::Number(k.to_string())
        )])).unwrap()
    }).collect()).unwrap()
}

fn schemas() -> (RelationCatalog, RelationCatalog) {
    let direct = RelationCatalog::from_schemas(&[
        RelationSchema::new(RELATION, vec![
            SchemaColumn::from_sql_type("k","INTEGER","postgresql").unwrap()
        ]).unwrap()
    ]).unwrap();
    let known = RelationCatalog::new(&[RELATION]).unwrap();
    let parsed = parse_odcs_yaml(r#"
apiVersion: v3.2.0
kind: DataContract
id: task136samples
name: Sampling evidence source
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: k
        physicalType: INTEGER
"#, "postgresql", &known).unwrap();
    let odcs = RelationCatalog::from_schemas(parsed.source_schemas()).unwrap();
    (direct, odcs)
}

fn dbt_manifest(sql: &str) -> String {
    json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8", "adapter_type": "postgres"
        },
        "nodes": {
            "model.demo.task136": {
                "unique_id": "model.demo.task136", "resource_type": "model",
                "relation_name": "warehouse.analytics.task136",
                "database": "warehouse", "schema": "analytics",
                "original_file_path": "models/task136.sql", "language": "sql",
                "raw_code": sql, "compiled_code": sql,
                "depends_on": {"nodes": ["source.demo.s"]}
            }
        },
        "sources": {
            "source.demo.s": {
                "unique_id": "source.demo.s", "resource_type": "source",
                "relation_name": RELATION, "database": "warehouse", "schema": "raw",
                "name": "s", "columns": {"k": {"name":"k", "data_type":"INTEGER"}}
            }
        }
    }).to_string()
}

#[test]
fn sampling_and_deterministic_page_fact_parity_from_all_three_evidence_sources() {
    let (direct_schema, odcs_schema) = schemas();
    let parser = dialect_from_name("postgresql").unwrap();
    let cases = [
        ("SELECT k FROM warehouse.raw.s ORDER BY k LIMIT 2", true, false),
        ("SELECT k FROM warehouse.raw.s ORDER BY k FETCH FIRST 2 ROWS ONLY", true, false),
        ("SELECT k FROM warehouse.raw.s TABLESAMPLE SYSTEM (50) ORDER BY k LIMIT 2", false, true),
        ("SELECT k FROM warehouse.raw.s TABLESAMPLE BERNOULLI (50) REPEATABLE (7)", false, true),
    ];
    let mut total = 0;
    for (sql, exact, sampling) in cases {
        let input = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new("task136", &input, "postgresql",parser.as_ref())];
        let direct = analyze_configured_inputs_with_catalog(&configured, &direct_schema).unwrap();
        let contract = analyze_configured_inputs_with_catalog(&configured, &odcs_schema).unwrap();
        let dbt = parse_dbt_manifest(&dbt_manifest(sql)).unwrap();
        let compiled = analyze_dbt_manifest_with_schemas(&dbt,"postgresql",parser.as_ref()).unwrap();

        let mut comparable: Vec<(Value, Value, Vec<String>, Vec<String>)> = Vec::new();
        for (origin, bundle) in [("sql",direct),("odcs",contract),("dbt",compiled)] {
            let layer = bundle.layers().last().unwrap().id();
            let proof = physical_finite_page_bag_plan(&bundle,layer,&[source()]);
            let (terminal, selected, rejected) = match proof {
                PhysicalFinitePageProof::Exact(complete) if exact => {
                    assert_eq!(complete.source,RELATION,"{origin}");
                    let values = complete.terminal.iter().map(|row| match &row["k"] {
                        LiteralValue::Number(k) => k.parse::<i64>().unwrap(),
                        other => panic!("{other:?}"),
                    }).collect::<Vec<_>>();
                    assert_eq!(values,vec![1,2],"{origin}");
                    let selected = complete.rows.iter().filter(|row|row.survives)
                        .map(|row|row.source_row_id.clone()).collect::<Vec<_>>();
                    let rejected = complete.rows.iter().filter(|row|!row.survives)
                        .map(|row|row.source_row_id.clone()).collect::<Vec<_>>();
                    (json!(values),selected,rejected)
                }
                PhysicalFinitePageProof::Residual { .. } if sampling =>
                    (Value::Null,vec![],vec![]),
                other => panic!("{origin}: unexpected physical proof: {other:?}"),
            };
            let emitted: Value = serde_json::from_str(&to_bundle_json(&bundle)).unwrap();
            let sample = emitted["inputs"][0]["statements"][0]["sampling"].clone();
            assert_eq!(sample.is_array(),sampling,"{origin}: canonical sample not preserved");
            if sampling {
                assert_eq!(sample[0]["membership"], "residual","{origin}");
                assert_eq!(sample[0]["source"],RELATION,"{origin}");
            }
            comparable.push((sample,terminal,selected,rejected));
            total += 1;
        }
        assert_eq!(comparable[0],comparable[1],"direct/ODCS parity: {sql}");
        assert_eq!(comparable[0],comparable[2],"direct/dbt parity: {sql}");
    }
    assert_eq!(total,12);
}
