//! TASK-144 task-owned adapter parity: compiled dbt SQL and ODCS schema
//! provide the *same* canonical CTE bags, provenance, and terminal domains.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_finite_cte_bag_plan, AnalysisBundle,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn cases() -> Value {
    serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap()
}

fn direct(sql: &str, catalog: &RelationCatalog) -> AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let source = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "adapter",
        &source,
        "duckdb",
        dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, catalog).unwrap()
}

fn catalog(source: bool) -> RelationCatalog {
    if !source {
        return RelationCatalog::from_schemas(&[]).unwrap();
    }
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "raw".to_string(),
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
    )
    .unwrap()])
    .unwrap()
}

fn snapshots(case: &Value) -> Vec<PhysicalWriteState> {
    case["initial_bags"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, rows)| {
            PhysicalWriteState::new(
                name.clone(),
                rows.as_array()
                    .unwrap()
                    .iter()
                    .map(|value| {
                        let literal = value
                            .as_i64()
                            .map_or(LiteralValue::Null, |v| LiteralValue::Number(v.to_string()));
                        PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), literal)])).unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect()
}

fn exact(
    bundle: &AnalysisBundle,
    states: &[PhysicalWriteState],
) -> sql_semantic_protocol::PhysicalCteExact {
    let layer = bundle.layers().last().unwrap();
    let result = physical_finite_cte_bag_plan(bundle, layer.id(), states, None);
    let PhysicalCteProof::Exact(proof) = result else {
        panic!("missing complete source-independent CTE proof: {result:?}");
    };
    *proof
}

fn compiled(sql: &str, with_source: bool) -> AnalysisBundle {
    let sources = if with_source { &["raw"][..] } else { &[][..] };
    compiled_named(sql, sources)
}

fn compiled_named(sql: &str, names: &[&str]) -> AnalysisBundle {
    let mut sources = serde_json::Map::new();
    let mut catalog_sources = serde_json::Map::new();
    let mut dependencies = Vec::new();
    for name in names {
        let id = format!("source.demo.{name}");
        dependencies.push(id.clone());
        let mut declared = serde_json::Map::new();
        let mut introspected = serde_json::Map::new();
        let columns = if *name == "target" { vec!["k","shifted"] } else { vec!["k"] };
        for (index, column) in columns.iter().enumerate() {
            declared.insert((*column).to_string(), json!({"name":column,"data_type":"INTEGER"}));
            introspected.insert((*column).to_string(), json!({"name":column,"type":"INTEGER","index":index+1}));
        }
        sources.insert(id.clone(), json!({
            "unique_id":id,"resource_type":"source","relation_name":name,
            "name":name,"columns":declared,
        }));
        catalog_sources.insert(id.clone(), json!({
            "unique_id":id,
            "metadata":{"type":"BASE TABLE","database":"main","schema":"raw","name":name},
            "columns":introspected,"stats":{}
        }));
    }
    let manifest = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8","adapter_type":"duckdb"},
        "nodes":{"model.demo.task144":{
            "unique_id":"model.demo.task144","resource_type":"model",
            "relation_name":"main.task144","name":"task144",
            "original_file_path":"models/task144.sql","language":"sql",
            "raw_code":"SELECT 0 WHERE FALSE","compiled_code":sql,
            "depends_on":{"nodes":dependencies}
        }},
        "sources":sources
    });
    let dbt_catalog = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version":"1.11.8"},
        "nodes":{},"sources":catalog_sources,"errors":null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let dbt_catalog = parse_dbt_catalog(&dbt_catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    analyze_dbt_artifacts(&manifest, &dbt_catalog, "duckdb", dialect.as_ref()).unwrap()
}

#[test]
fn compiled_dbt_exactly_matches_sql_for_every_representable_frozen_read() {
    let freeze = cases();
    let mut checked = Vec::new();
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if !matches!(
            id,
            "78-C144-P-REUSED-FILTER" | "78-C144-N-NULL-DUP" | "78-C144-P-FINITE-RECURSION"
        ) {
            continue;
        }
        let with_source = !id.contains("RECURSION");
        let sql = case["sql"].as_str().unwrap();
        let states = snapshots(case);
        let direct = exact(&direct(sql, &catalog(with_source)), &states);
        let dbt = exact(&compiled(sql, with_source), &states);
        assert_eq!(dbt, direct, "{id}: exact stages/source identities/domains");
        checked.push(id);
    }
    checked.sort();
    assert_eq!(
        checked,
        [
            "78-C144-N-NULL-DUP",
            "78-C144-P-FINITE-RECURSION",
            "78-C144-P-REUSED-FILTER"
        ]
    );
}

#[cfg(feature = "odcs")]
#[test]
fn odcs_equivalent_source_schema_preserves_complete_task_144_bags() {
    use sql_semantic_protocol::parse_odcs_yaml;
    let contract = concat!(
        "apiVersion: v3.2.0\n",
        "kind: DataContract\n",
        "id: task144\n",
        "name: CTE producer source schema\n",
        "version: 1.0.0\n",
        "status: active\n",
        "schema:\n",
        "  - name: raw\n    physicalName: raw\n    properties:\n",
        "      - name: k\n        physicalType: INTEGER\n",
    );
    let metadata =
        parse_odcs_yaml(contract, "duckdb", &RelationCatalog::new(&["raw"]).unwrap()).unwrap();
    let evidence = RelationCatalog::from_schemas(metadata.source_schemas()).unwrap();
    let freeze = cases();
    let mut checked = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if !matches!(id, "78-C144-P-REUSED-FILTER" | "78-C144-N-NULL-DUP") {
            continue;
        }
        let sql = case["sql"].as_str().unwrap();
        let states = snapshots(case);
        assert_eq!(
            exact(&direct(sql, &evidence), &states),
            exact(&direct(sql, &catalog(true)), &states),
            "{id}: ODCS gives equivalent canonical physical SQL evidence"
        );
        checked += 1;
    }
    assert_eq!(checked, 2);
}
