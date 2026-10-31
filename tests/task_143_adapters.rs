//! TASK-143 complete physical SQL/dbt/ODCS adapter parity for the frozen CTE cases.
//! TASK-141 direct SQL and compiled dbt lexical-source parity.
//! Adapter parity is about canonical source identities, not an independent
//! proof that a complete physical CTE evaluator exists.

use serde_json::{json, Map, Value};
use sql_semantic_protocol::{
    dialect_from_name, parse_dbt_manifest, AnalysisBundle, ProtocolStatement,
    QueryStatement, SqlInput,
};

fn compiled_dbt(sql: &str, initial_bags: &Value, consumed_source: &str) -> AnalysisBundle {
    let mut sources = Map::new();
    let mut dependencies = Vec::new();
    for name in initial_bags.as_object().unwrap().keys() {
        let unique_id = format!("source.demo.{name}");
        if name == consumed_source {
            dependencies.push(unique_id.clone());
        }
        sources.insert(
            unique_id.clone(),
            json!({
                "unique_id": unique_id,
                "resource_type": "source",
                "relation_name": name,
                "name": name,
                "columns": {"k": {"name": "k", "data_type": "INTEGER"}}
            }),
        );
    }
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "duckdb"
        },
        "nodes": {"model.demo.task143": {
            "unique_id": "model.demo.task143",
            "resource_type": "model",
            "relation_name": "main.task143",
            "name": "task143",
            "original_file_path": "models/task143.sql",
            "language": "sql",
            "raw_code": "SELECT 0 WHERE FALSE",
            "compiled_code": sql,
            "depends_on": {"nodes": dependencies}
        }},
        "sources": sources
    });
    let parsed = parse_dbt_manifest(&manifest.to_string()).unwrap();
    // Catalog evidence covers all physical leaves, even those contributing
    // only to an unused, shadowed outer CTE.
    let mut catalog_sources = Map::new();
    for name in initial_bags.as_object().unwrap().keys() {
        let id = format!("source.demo.{name}");
        catalog_sources.insert(
            id.clone(),
            json!({
                "unique_id": id,
                "metadata": {
                    "type": "BASE TABLE", "database": "main",
                    "schema": "raw", "name": name
                },
                "columns": {
                    "k": {"name": "k", "type": "INTEGER", "index": 1}
                },
                "stats": {}
            }),
        );
    }
    let catalog = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version": "1.11.8"
        },
        "nodes": {},
        "sources": catalog_sources,
        "errors": null
    });
    let parsed_catalog = sql_semantic_protocol::parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    sql_semantic_protocol::analyze_dbt_artifacts(
        &parsed,
        &parsed_catalog,
        "duckdb",
        dialect.as_ref(),
    )
    .unwrap()
}

fn model_query(bundle: &AnalysisBundle) -> &QueryStatement {
    bundle
        .inputs()
        .iter()
        .flat_map(|input| input.statements())
        .find_map(|statement| match statement {
            ProtocolStatement::Query(query) => Some(query),
            _ => None,
        })
        .expect("compiled model query")
}

fn complete_states(input: &Value) -> Vec<sql_semantic_protocol::PhysicalWriteState> {
    input
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, rows)| {
            let rows = rows
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let value = match row["k"].as_i64() {
                        Some(n) => sql_semantic_protocol::LiteralValue::Number(n.to_string()),
                        None => sql_semantic_protocol::LiteralValue::Null,
                    };
                    sql_semantic_protocol::PhysicalWriteRow::new(std::collections::BTreeMap::from(
                        [("k".to_string(), value)],
                    ))
                    .unwrap()
                })
                .collect();
            sql_semantic_protocol::PhysicalWriteState::new(name.clone(), rows).unwrap()
        })
        .collect()
}

fn attested_direct(sql: &str, inputs: &Value) -> sql_semantic_protocol::AnalysisBundle {
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, ConfiguredSqlInput, RelationCatalog,
        RelationSchema, SchemaColumn,
    };
    let schemas = inputs
        .as_object()
        .unwrap()
        .keys()
        .map(|name| {
            RelationSchema::new(
                name.clone(),
                vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "task143",
        &input,
        "duckdb",
        dialect.as_ref(),
    )];
    analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap()
}

fn physical_bags(
    bundle: &AnalysisBundle,
    states: &[sql_semantic_protocol::PhysicalWriteState],
) -> sql_semantic_protocol::PhysicalCteExact {
    use sql_semantic_protocol::{physical_finite_cte_bag_plan, PhysicalCteProof};
    let layer = bundle.layers().last().unwrap();
    let result = physical_finite_cte_bag_plan(bundle, layer.id(), states, None);
    let PhysicalCteProof::Exact(value) = result else {
        panic!("adapter lacks source-complete CTE physical proof: {result:?}");
    };
    *value
}

#[test]
fn each_frozen_task_143_read_case_has_identical_sql_and_dbt_physical_proofs() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-143-acceptance-freeze.json")).unwrap();
    let mut checked = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" || !case["sql"].is_string() || case["id"].as_str().unwrap().contains("WRITE") {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let direct = attested_direct(sql, &case["initial_bags"]);
        let dbt = compiled_dbt(sql, &case["initial_bags"], "raw");
        let states = complete_states(&case["initial_bags"]);
        assert!(model_query(&direct).cte_program().is_some(), "{id}/direct");
        assert!(model_query(&dbt).cte_program().is_some(), "{id}/dbt");
        assert_eq!(
            model_query(&direct).dependencies(),
            model_query(&dbt).dependencies(),
            "{id}: physical dependency parity"
        );
        let direct = physical_bags(&direct, &states);
        let dbt = physical_bags(&dbt, &states);
        assert_eq!(dbt, direct, "{id}: complete physical bags, TRUE/NOT TRUE, exact domains and identities");
        checked += 1;
    }
    assert_eq!(checked, 3);
}

#[cfg(feature = "odcs")]
#[test]
fn each_frozen_task_143_read_case_has_identical_odcs_schema_evidence() {
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, parse_odcs_yaml, ConfiguredSqlInput,
        RelationCatalog,
    };
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-143-acceptance-freeze.json")).unwrap();
    let catalog = RelationCatalog::new(&["raw"]).unwrap();
    let contract = concat!(
        "apiVersion: v3.2.0\n",
        "kind: DataContract\n",
        "id: task143\n",
        "name: CTE producer source schema\n",
        "version: 1.0.0\n",
        "status: active\n",
        "schema:\n",
        "  - name: raw\n    physicalName: raw\n    properties:\n",
        "      - name: k\n        physicalType: INTEGER\n"
    );
    let parsed = parse_odcs_yaml(contract, "duckdb", &catalog).unwrap();
    let evidence = RelationCatalog::from_schemas(parsed.source_schemas()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let mut checked = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" || !case["sql"].is_string() || case["id"].as_str().unwrap().contains("WRITE") {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let source = SqlInput::inline(case["sql"].as_str().unwrap());
        let configured = [ConfiguredSqlInput::new("task143", &source, "duckdb", dialect.as_ref())];
        let odcs = analyze_configured_inputs_with_catalog(&configured, &evidence).unwrap();
        let direct = attested_direct(case["sql"].as_str().unwrap(), &case["initial_bags"]);
        let states = complete_states(&case["initial_bags"]);
        assert_eq!(
            physical_bags(&odcs, &states),
            physical_bags(&direct, &states),
            "{id}: ODCS schema must preserve complete source/producers/terminal bags"
        );
        checked += 1;
    }
    assert_eq!(checked, 3);
}
