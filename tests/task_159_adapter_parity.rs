//! TASK-159 SQL owns lifecycle; dbt and ODCS own equivalent typed schema facts.
//! A compiled dbt model cannot be silently treated as an executable DROP.

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_ordered_write_state_plan, ConfiguredSqlInput,
    PhysicalWriteStatePlan, RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WriteKind,
};

fn direct() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "t",
        vec![
            SchemaColumn::from_sql_type("a", "INTEGER", "duckdb").unwrap(),
            SchemaColumn::from_sql_type("b", "INTEGER", "duckdb").unwrap(),
        ],
    )
    .unwrap()])
    .unwrap()
}

fn dbt_bundle(compiled_sql: &str) -> Result<sql_semantic_protocol::AnalysisBundle, String> {
    let manifest = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                    "dbt_version":"1.11.8","adapter_type":"duckdb"},
        "nodes": {
            "model.task159.probe": {
                "unique_id":"model.task159.probe","resource_type":"model",
                "name":"probe","relation_name":"task159_probe",
                "original_file_path":"models/probe.sql","language":"sql",
                "raw_code":compiled_sql,"compiled_code":compiled_sql,
                "depends_on":{"nodes":["source.task159.t"]}
            }
        },
        "sources":{
            "source.task159.t":{
                "unique_id":"source.task159.t","resource_type":"source",
                "relation_name":"t","name":"t",
                "columns":{
                    "a":{"name":"a","data_type":"INTEGER"},
                    "b":{"name":"b","data_type":"INTEGER"}
                }
            }
        }
    });
    let catalog = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
                    "dbt_version":"1.11.8"},
        "nodes":{},
        "sources":{
            "source.task159.t":{
                "unique_id":"source.task159.t",
                "metadata":{"type":"BASE TABLE","database":"main","schema":"main","name":"t"},
                "columns":{
                    "a":{"name":"a","type":"INTEGER","index":1},
                    "b":{"name":"b","type":"INTEGER","index":2}
                },
                "stats":{}
            }
        },
        "errors":null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref())
        .map_err(|e| e.to_string())
}

fn dbt() -> RelationCatalog {
    let analyzed = dbt_bundle("SELECT a FROM t").unwrap();
    RelationCatalog::from_schemas(analyzed.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let contract = "apiVersion: v3.2.0\nkind: DataContract\nid: task159\nname: TASK-159 parity\nversion: 1.0.0\nstatus: active\nschema:\n  - name: t\n    physicalName: t\n    properties:\n      - name: a\n        physicalType: INTEGER\n      - name: b\n        physicalType: INTEGER\n";
    let evidence =
        parse_odcs_yaml(contract, "duckdb", &RelationCatalog::new(&["t"]).unwrap()).unwrap();
    RelationCatalog::from_schemas(evidence.source_schemas()).unwrap()
}

fn replay(catalog: &RelationCatalog, lifecycle: &str) -> (String, String, bool, String) {
    let parser = dialect_from_name("duckdb").unwrap();
    let source = [
        SqlInput::inline("CREATE TABLE t (a INTEGER, b INTEGER)"),
        SqlInput::inline("INSERT INTO t (a, b) VALUES (1, NULL), (1, NULL)"),
        SqlInput::inline(lifecycle),
    ];
    let ids = ["create", "insert", "lifecycle"];
    let configured = source
        .iter()
        .zip(ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "duckdb", parser.as_ref()))
        .collect::<Vec<_>>();
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    let last = &bundle.inputs()[2].statements()[0];
    let sql_semantic_protocol::ProtocolStatement::Query(query) = last else {
        panic!("SQL lifecycle input must have a canonical write");
    };
    assert_eq!(query.write().unwrap().kind(), WriteKind::Lifecycle);
    match physical_ordered_write_state_plan(&bundle, &[]) {
        PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } => {
            let terminal = transitions.last().unwrap();
            let after_schema = terminal
                .after_schema()
                .map(|schema| {
                    schema
                        .columns()
                        .iter()
                        .map(|column| format!("{}:{:?}", column.name(), column.data_type()))
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            (
                format!("{:?}", terminal.before().rows()),
                format!("{:?}", final_states),
                terminal.after_exists(),
                after_schema,
            )
        }
        PhysicalWriteStatePlan::Residual { gap, .. } => (
            format!("Residual:{gap:?}"),
            String::new(),
            false,
            String::new(),
        ),
        PhysicalWriteStatePlan::Impossible { .. } => (
            "Impossible".to_string(),
            String::new(),
            false,
            String::new(),
        ),
    }
}

#[test]
fn direct_sql_and_dbt_catalog_agree_on_lifecycle_state_and_schema() {
    for sql in [
        "TRUNCATE TABLE t",
        "DROP TABLE t",
        "DROP TABLE IF EXISTS t",
        "ALTER TABLE t RENAME TO u",
    ] {
        assert_eq!(replay(&direct(), sql), replay(&dbt(), sql), "{sql}");
    }
}

#[cfg(feature = "odcs")]
#[test]
fn direct_sql_and_odcs_agree_on_lifecycle_state_and_schema() {
    for sql in [
        "TRUNCATE TABLE t",
        "DROP TABLE t",
        "DROP TABLE IF EXISTS t",
        "ALTER TABLE t RENAME TO u",
    ] {
        assert_eq!(replay(&direct(), sql), replay(&odcs(), sql), "{sql}");
    }
}

#[test]
fn dbt_compiled_model_drop_is_explicitly_not_a_select_backed_lifecycle() {
    let error = dbt_bundle("DROP TABLE t").unwrap_err();
    assert!(
        error.contains("Expected: SELECT") && error.contains("DROP"),
        "{error}"
    );
}


fn replay_known_absence(catalog: &RelationCatalog, conditional: bool) -> String {
    let parser = dialect_from_name("duckdb").unwrap();
    let last = if conditional { "DROP TABLE IF EXISTS t" } else { "DROP TABLE t" };
    let source = [
        SqlInput::inline("CREATE TABLE t (a INTEGER, b INTEGER)"),
        SqlInput::inline("INSERT INTO t (a, b) VALUES (1, NULL), (1, NULL)"),
        SqlInput::inline("DROP TABLE t"),
        SqlInput::inline(last),
    ];
    let ids = ["create", "insert", "drop", "conditional"];
    let configured = source.iter().zip(ids).map(|(input, id)| {
        ConfiguredSqlInput::new(id, input, "duckdb", parser.as_ref())
    }).collect::<Vec<_>>();
    let analyzed = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    match physical_ordered_write_state_plan(&analyzed, &[]) {
        PhysicalWriteStatePlan::Feasible { transitions, final_states } => {
            assert!(conditional);
            assert!(final_states.is_empty());
            let last = transitions.last().unwrap();
            assert!(!last.before_exists() && !last.after_exists());
            format!("Exact:{}:{}", transitions.len(), last.affected().deleted())
        }
        PhysicalWriteStatePlan::Impossible { layer_id, .. } => {
            assert!(!conditional);
            format!("Impossible:{}", layer_id.unwrap())
        }
        PhysicalWriteStatePlan::Residual { gap, .. } => format!("Residual:{gap:?}"),
    }
}

#[test]
fn direct_and_dbt_agree_on_proven_missing_if_exists_and_missing_drop_refutation() {
    for conditional in [false, true] {
        let expected = replay_known_absence(&direct(), conditional);
        assert_eq!(expected, if conditional { "Exact:4:0" } else { "Impossible:layer-0004" });
        assert_eq!(expected, replay_known_absence(&dbt(), conditional));
        #[cfg(feature = "odcs")]
        assert_eq!(expected, replay_known_absence(&odcs(), conditional));
    }
}
