//! TASK-141 direct SQL and compiled dbt lexical-source parity.
//! Adapter parity is about canonical source identities, not an independent
//! proof that a complete physical CTE evaluator exists.

use serde_json::{json, Map, Value};
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, parse_dbt_manifest, AnalysisBundle, ProtocolStatement,
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
        "nodes": {"model.demo.task141": {
            "unique_id": "model.demo.task141",
            "resource_type": "model",
            "relation_name": "main.task141",
            "name": "task141",
            "original_file_path": "models/task141.sql",
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

#[test]
fn frozen_cte_source_instances_match_direct_and_compiled_dbt() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let mut cases = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let direct = analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref()).unwrap();
        let consumed_source = if id.ends_with("SHADOW") {
            "inner_src"
        } else {
            "s"
        };
        let compiled = compiled_dbt(sql, &case["initial_bags"], consumed_source);
        let direct = model_query(&direct);
        let compiled = model_query(&compiled);
        assert_eq!(
            compiled.dependencies(),
            direct.dependencies(),
            "{id}: physical dependencies"
        );
        assert_eq!(
            compiled.sources(),
            direct.sources(),
            "{id}: distinct source instances"
        );
        assert_eq!(
            compiled
                .output()
                .columns()
                .iter()
                .map(|column| column.name())
                .collect::<Vec<_>>(),
            direct
                .output()
                .columns()
                .iter()
                .map(|column| column.name())
                .collect::<Vec<_>>(),
            "{id}: output identities"
        );
        cases += 1;
    }
    assert_eq!(cases, 4);
}

#[cfg(feature = "odcs")]
#[test]
fn equivalent_odcs_source_schema_preserves_frozen_cte_source_identities() {
    use sql_semantic_protocol::{
        analyze_configured_inputs_with_catalog, parse_odcs_yaml, ConfiguredSqlInput,
        RelationCatalog,
    };

    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let mut checked = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        let id = case["id"].as_str().unwrap();
        let sql = case["sql"].as_str().unwrap();
        let source_names = case["initial_bags"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let catalog = RelationCatalog::new(&source_names).unwrap();
        let mut contract = concat!(
            "apiVersion: v3.2.0\n",
            "kind: DataContract\n",
            "id: task141\n",
            "name: CTE source schema\n",
            "version: 1.0.0\n",
            "status: active\n",
            "schema:\n"
        )
        .to_string();
        for name in &source_names {
            contract.push_str(&format!(
                "  - name: {name}\n    physicalName: {name}\n    properties:\n      - name: k\n        physicalType: INTEGER\n"
            ));
        }
        let parsed = parse_odcs_yaml(&contract, "duckdb", &catalog)
            .unwrap_or_else(|error| panic!("{id}: ODCS contract: {error}"));
        let evidence = RelationCatalog::from_schemas(parsed.source_schemas()).unwrap();
        let source = SqlInput::inline(sql);
        let configured = [ConfiguredSqlInput::new(
            "task141",
            &source,
            "duckdb",
            dialect.as_ref(),
        )];
        let from_contract = analyze_configured_inputs_with_catalog(&configured, &evidence)
            .unwrap_or_else(|error| panic!("{id}: ODCS analysis: {error}"));
        let from_sql =
            analyze_inputs(&[SqlInput::inline(sql)], "duckdb", dialect.as_ref()).unwrap();
        let from_contract_bundle = from_contract.clone();
        let from_contract = model_query(&from_contract);
        let from_sql = model_query(&from_sql);
        assert_eq!(
            from_contract.dependencies(),
            from_sql.dependencies(),
            "{id}: ODCS source schema cannot change CTE source identity"
        );
        assert_eq!(
            from_contract.sources(),
            from_sql.sources(),
            "{id}: ODCS source schema cannot create or hide a local instance"
        );
        let direct = attested_direct(sql, &case["initial_bags"]);
        let states = complete_states(&case["initial_bags"]);
        assert!(model_query(&direct).cte_program().is_some(), "{id}: direct SQL has no CTE program");
        assert!(model_query(&dbt).cte_program().is_some(), "{id}: dbt compiled SQL has no CTE program");
        let direct = physical_bags(&direct, &states);
        let odcs = physical_bags(&from_contract_bundle, &states);
        assert_eq!(
            odcs.source_bags, direct.source_bags,
            "{id}: ODCS physical leaves"
        );
        assert_eq!(
            odcs.stage_bags, direct.stage_bags,
            "{id}: ODCS producer stages"
        );
        assert_eq!(odcs.rejected, direct.rejected, "{id}: ODCS NOT TRUE");
        assert_eq!(
            odcs.terminal_domains, direct.terminal_domains,
            "{id}: ODCS domains"
        );
        checked += 1;
    }
    assert_eq!(checked, 4);
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
        "task141",
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
fn frozen_cte_physical_bags_match_direct_and_dbt_adapters() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-141-acceptance-freeze.json")).unwrap();
    let mut verified = 0;
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" {
            continue;
        }
        let sql = case["sql"].as_str().unwrap();
        let id = case["id"].as_str().unwrap();
        let consumed_source = if id.ends_with("SHADOW") {
            "inner_src"
        } else {
            "s"
        };
        let direct = attested_direct(sql, &case["initial_bags"]);
        let dbt = compiled_dbt(sql, &case["initial_bags"], consumed_source);
        let states = complete_states(&case["initial_bags"]);
        let direct = physical_bags(&direct, &states);
        let dbt = physical_bags(&dbt, &states);
        assert_eq!(
            direct.source_bags, dbt.source_bags,
            "{id}: source row identity"
        );
        assert_eq!(direct.stage_bags, dbt.stage_bags, "{id}: CTE producer bags");
        assert_eq!(direct.rejected, dbt.rejected, "{id}: SQL NOT TRUE");
        assert_eq!(direct.false_pairs, dbt.false_pairs, "{id}: SQL FALSE");
        assert_eq!(direct.unknown_pairs, dbt.unknown_pairs, "{id}: SQL UNKNOWN");
        assert_eq!(direct.terminal, dbt.terminal, "{id}: terminal bag");
        assert_eq!(
            direct.terminal_domains, dbt.terminal_domains,
            "{id}: typed bounds"
        );
        verified += 1;
    }
    assert_eq!(verified, 4);
}
