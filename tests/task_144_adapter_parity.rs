//! TASK-144 task-owned adapter parity: compiled dbt SQL and ODCS schema
//! provide the *same* canonical CTE bags, provenance, and terminal domains.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_finite_cte_bag_plan,
    physical_finite_view_read_plan, physical_ordered_cte_read_plan, AnalysisBundle,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalOrderedCteProof, PhysicalViewProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
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
    compiled_with_dependencies(sql, names, names)
}

fn compiled_with_dependencies(
    sql: &str,
    names: &[&str],
    referenced: &[&str],
) -> AnalysisBundle {
    let mut sources = serde_json::Map::new();
    let mut catalog_sources = serde_json::Map::new();
    let dependencies = referenced.iter().map(|name| format!("source.demo.{name}")).collect::<Vec<_>>();
    for name in names {
        let id = format!("source.demo.{name}");
        let mut declared = serde_json::Map::new();
        let mut introspected = serde_json::Map::new();
        let columns = if *name == "target" {
            vec!["k", "shifted"]
        } else {
            vec!["k"]
        };
        for (index, column) in columns.iter().enumerate() {
            declared.insert(
                (*column).to_string(),
                json!({"name":column,"data_type":"INTEGER"}),
            );
            introspected.insert(
                (*column).to_string(),
                json!({"name":column,"type":"INTEGER","index":index+1}),
            );
        }
        sources.insert(
            id.clone(),
            json!({
                "unique_id":id,"resource_type":"source","relation_name":name,
                "name":name,"columns":declared,
            }),
        );
        catalog_sources.insert(
            id.clone(),
            json!({
                "unique_id":id,
                "metadata":{"type":"BASE TABLE","database":"main","schema":"raw","name":name},
                "columns":introspected,"stats":{}
            }),
        );
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

#[test]
fn shadowed_cte_has_same_two_source_physical_proof_in_compiled_dbt() {
    let freeze = cases();
    let case = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "78-C144-P-SHADOW")
        .unwrap();
    let source_names = ["inner_src", "outer_src"];
    let schemas = source_names
        .iter()
        .map(|name| {
            RelationSchema::new(
                (*name).to_string(),
                vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let evidence = RelationCatalog::from_schemas(&schemas).unwrap();
    let sql = case["sql"].as_str().unwrap();
    let states = snapshots(case);
    assert_eq!(
        exact(&direct(sql, &evidence), &states),
        exact(&compiled_with_dependencies(sql, &source_names, &["inner_src"]), &states),
        "compiled dbt nested shadowing must preserve BOTH complete leaves and lexical identities"
    );
}

#[cfg(feature = "odcs")]
#[test]
fn shadowed_cte_odcs_sources_preserve_complete_dual_leaf_truth() {
    use sql_semantic_protocol::parse_odcs_yaml;
    let contract = concat!(
        "apiVersion: v3.2.0\nkind: DataContract\n",
        "id: task144-shadow\nname: Two physical leaves\n",
        "version: 1.0.0\nstatus: active\nschema:\n",
        "  - name: inner_src\n    physicalName: inner_src\n    properties:\n",
        "      - name: k\n        physicalType: INTEGER\n",
        "  - name: outer_src\n    physicalName: outer_src\n    properties:\n",
        "      - name: k\n        physicalType: INTEGER\n"
    );
    let parsed = parse_odcs_yaml(
        contract,
        "duckdb",
        &RelationCatalog::new(&["inner_src", "outer_src"]).unwrap(),
    )
    .unwrap();
    let evidence = RelationCatalog::from_schemas(parsed.source_schemas()).unwrap();
    let case = cases()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "78-C144-P-SHADOW")
        .unwrap()
        .clone();
    let states = snapshots(&case);
    assert_eq!(
        exact(&direct(case["sql"].as_str().unwrap(), &evidence), &states),
        exact(
            &compiled_with_dependencies(case["sql"].as_str().unwrap(), &["inner_src", "outer_src"], &["inner_src"]),
            &states
        ),
        "ODCS and compiled dbt must preserve shadowed source identity and full bag"
    );
}

fn view_script(catalog: &RelationCatalog, statements: &[Value]) -> AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let source = statements
        .iter()
        .map(|sql| SqlInput::inline(sql.as_str().unwrap()))
        .collect::<Vec<_>>();
    let configured = source
        .iter()
        .enumerate()
        .map(|(index, sql)| {
            let name = if index == 0 { "view" } else { "terminal" };
            ConfiguredSqlInput::new(name, sql, "duckdb", dialect.as_ref())
        })
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, catalog).unwrap()
}

#[test]
fn compiled_dbt_view_model_has_exactly_the_logical_views_source_derived_bag() {
    let freeze = cases();
    let case = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "78-C144-P-VIEW")
        .unwrap();
    let sqls = case["sql_statements"].as_array().unwrap();
    let source = snapshots(case);
    let direct_bundle = view_script(&catalog(true), sqls);
    let terminal = direct_bundle.layers().last().unwrap();
    let PhysicalViewProof::Exact(view) =
        physical_finite_view_read_plan(&direct_bundle, terminal.id(), &source, None)
    else {
        panic!("native VIEW must be source-derived rather than independently inserted");
    };
    // The compiled dbt model SQL is the authoritative manifest fact; a dbt
    // materialization configuration is not a substitute for the compiled query.
    let dbt_sql = "WITH stage AS (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1) SELECT k, shifted FROM stage";
    let dbt = exact(&compiled_named(dbt_sql, &["raw"]), &source);
    assert_eq!(dbt.sources, vec!["raw"]);
    assert_eq!(dbt.terminal, view.view_bag, "dbt compiled view query and CREATE VIEW have identical canonical value bags and physical source ids");
    assert_eq!(
        dbt.terminal_domains["shifted"].integer_bounds,
        Some((12, 13))
    );
    assert!(!dbt.terminal_domains["shifted"].null_allowed);
}

#[test]
fn compiled_dbt_read_preserves_the_complete_raw_script_materialized_poststate() {
    let freeze = cases();
    for id in ["78-C144-P-MULTISTEP", "78-C144-N-MULTISTEP-NULL"] {
        let case = freeze["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        let catalog = RelationCatalog::from_schemas(&[
            RelationSchema::new(
                "raw".to_string(),
                vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
            )
            .unwrap(),
            RelationSchema::new(
                "target".to_string(),
                vec![
                    SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap(),
                    SchemaColumn::from_sql_type("shifted", "INTEGER", "duckdb").unwrap(),
                ],
            )
            .unwrap(),
        ])
        .unwrap();
        let source = case["sql_statements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sql| SqlInput::inline(sql.as_str().unwrap()))
            .collect::<Vec<_>>();
        let dialect = dialect_from_name("duckdb").unwrap();
        let configured = source
            .iter()
            .enumerate()
            .map(|(index, sql)| {
                ConfiguredSqlInput::new(
                    if index == 0 { "insert" } else { "read" },
                    sql,
                    "duckdb",
                    dialect.as_ref(),
                )
            })
            .collect::<Vec<_>>();
        let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
        let result = physical_ordered_cte_read_plan(
            &bundle,
            bundle.layers().last().unwrap().id(),
            &snapshots(case),
            None,
        );
        let PhysicalOrderedCteProof::Exact(proof) = result else {
            panic!("{id}: no original-source derived final state: {result:?}");
        };
        let target = proof
            .final_states
            .iter()
            .find(|state| state.relation() == "target")
            .unwrap();
        let dbt_read = compiled_named(source[1].sql(), &["target"]);
        let dbt = exact(&dbt_read, std::slice::from_ref(target));
        assert_eq!(dbt, *proof.read, "{id}: dbt compiled read must equal original-source constructed target/CTE/terminal bag");
    }
}
