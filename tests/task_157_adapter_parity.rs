//! TASK-157: equivalent typed SQL, dbt artifact and ODCS metadata must
//! produce the same CREATE/CTAS/MV and logical-view physical outcomes.
//! DDL SQL remains the source of executable lifecycle identity; metadata
//! adapters contribute schema only, not imaginary rows or objects.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_finite_view_read_plan,
    physical_ordered_write_state_plan, ConfiguredSqlInput, LiteralValue, PhysicalViewProof,
    PhysicalWriteRow, PhysicalWriteState, PhysicalWriteStatePlan, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-157-acceptance-freeze.json")).unwrap()
}

const NAMES: &[&str] = &[
    "raw",
    "empty_stage",
    "copy_stage",
    "stage.copy_stage",
    "view_stage",
    "snap_stage",
];

fn schema(name: &str) -> RelationSchema {
    RelationSchema::new(
        name,
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
    )
    .unwrap()
}

fn direct() -> RelationCatalog {
    RelationCatalog::from_schemas(&NAMES.iter().map(|name| schema(name)).collect::<Vec<_>>())
        .unwrap()
}

fn dbt() -> RelationCatalog {
    let mut sources = serde_json::Map::new();
    let mut introspected = serde_json::Map::new();
    for (index, name) in NAMES.iter().enumerate() {
        let key = format!("source.task157.s{index}");
        sources.insert(
            key.clone(),
            json!({
                "unique_id":key, "resource_type":"source", "relation_name":name, "name":name,
                "columns":{"k":{"name":"k","data_type":"INTEGER"}}
            }),
        );
        introspected.insert(key.clone(), json!({
            "unique_id":key, "metadata":{"type":"BASE TABLE", "database":"main", "schema":"main","name":name},
            "columns":{"k":{"name":"k","type":"INTEGER","index":1}}, "stats":{}
        }));
    }
    let manifest = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                    "dbt_version":"1.11.8","adapter_type":"duckdb"},
        "nodes":{"model.task157.probe":{
            "unique_id":"model.task157.probe", "resource_type":"model", "name":"probe",
            "relation_name":"task157_probe", "original_file_path":"models/probe.sql",
            "language":"sql", "raw_code":"SELECT k FROM raw", "compiled_code":"SELECT k FROM raw",
            "depends_on":{"nodes":["source.task157.s0"]}
        }},
        "sources":sources
    });
    let catalog = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
                    "dbt_version":"1.11.8"},
        "nodes":{}, "sources":introspected, "errors":null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let analyzed = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(analyzed.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs(case: &Value) -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;

    let mut contract = String::from(
        "apiVersion: v3.2.0\nkind: DataContract\nid: task157\nname: TASK-157 parity\nversion: 1.0.0\nstatus: active\nschema:\n",
    );
    let target = case["target"].as_str().unwrap();
    let names = ["raw", target];
    for name in names {
        contract.push_str(&format!(
            "  - name: {name}\n    physicalName: {name}\n    properties:\n      - name: k\n        physicalType: INTEGER\n"
        ));
    }
    let known = RelationCatalog::new(&names).unwrap();
    let evidence = parse_odcs_yaml(&contract, "duckdb", &known).unwrap();
    RelationCatalog::from_schemas(evidence.source_schemas()).unwrap()
}

fn bundle(case: &Value, catalog: &RelationCatalog) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = case["sql"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stmt| SqlInput::inline(stmt.as_str().unwrap()))
        .collect::<Vec<_>>();
    let ids = (0..inputs.len())
        .map(|index| format!("adapter-step-{index}"))
        .collect::<Vec<_>>();
    let configured = inputs
        .iter()
        .zip(&ids)
        .map(|(sql, id)| ConfiguredSqlInput::new(id, sql, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, catalog).unwrap()
}

fn initial(case: &Value) -> Vec<PhysicalWriteState> {
    if case["id"] == "83-C157-P-empty-table" {
        return Vec::new();
    }
    case["initial"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, values)| {
            PhysicalWriteState::new(
                name,
                values
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        PhysicalWriteRow::new(BTreeMap::from([(
                            "k".to_owned(),
                            row["k"].as_i64().map_or(LiteralValue::Null, |n| {
                                LiteralValue::Number(n.to_string())
                            }),
                        )]))
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect()
}

fn scoped_catalog(case: &Value, evidence: &RelationCatalog) -> RelationCatalog {
    let target = case["target"].as_str().unwrap();
    let schemas = evidence
        .schemas()
        .iter()
        .filter(|schema| schema.relation() == "raw" || schema.relation() == target)
        .cloned()
        .collect::<Vec<_>>();
    RelationCatalog::from_schemas(&schemas).unwrap()
}

fn outcome(case: &Value, catalog: &RelationCatalog) -> String {
    let analyzed = bundle(case, catalog);
    let initial = initial(case);
    let id = case["id"].as_str().unwrap();
    if id == "83-C157-P-logical-view" {
        let terminal = analyzed.layers().last().unwrap();
        let proof = physical_finite_view_read_plan(&analyzed, terminal.id(), &initial, None);
        assert!(
            matches!(proof, PhysicalViewProof::Exact(_)),
            "{id}: {proof:?}"
        );
        return format!("{proof:?}");
    }
    let plan = physical_ordered_write_state_plan(&analyzed, &initial);
    match case["classification"].as_str().unwrap() {
        "Exact" => assert!(
            matches!(plan, PhysicalWriteStatePlan::Feasible { .. }),
            "{id}: {plan:?}"
        ),
        "Impossible" => assert!(
            matches!(plan, PhysicalWriteStatePlan::Impossible { .. }),
            "{id}: {plan:?}"
        ),
        "Residual" => assert!(
            matches!(plan, PhysicalWriteStatePlan::Residual { .. }),
            "{id}: {plan:?}"
        ),
        other => panic!("{id}: unreviewed outcome class {other}"),
    }
    format!("{plan:?}")
}

#[test]
fn all_frozen_ddl_cases_preserve_exact_outcome_from_equivalent_dbt_schema() {
    let freeze = frozen();
    let direct = direct();
    let dbt = dbt();
    assert_eq!(freeze["coverage"]["adapter_cells_required"], 27);
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        assert_eq!(
            outcome(case, &scoped_catalog(case, &direct)),
            outcome(case, &scoped_catalog(case, &dbt)),
            "{id}: compiled dbt catalog parity"
        );
    }
}

#[cfg(feature = "odcs")]
#[test]
fn all_frozen_ddl_cases_preserve_exact_outcome_from_equivalent_odcs_schema() {
    let freeze = frozen();
    let direct = direct();
    for case in freeze["cases"].as_array().unwrap() {
        let odcs = odcs(case);
        let id = case["id"].as_str().unwrap();
        assert_eq!(
            outcome(case, &scoped_catalog(case, &direct)),
            outcome(case, &scoped_catalog(case, &odcs)),
            "{id}: ODCS canonical schema parity"
        );
    }
}
