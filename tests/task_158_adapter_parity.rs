//! TASK-158: equivalent direct SQL, dbt catalog and ODCS typed prestate.
//! Metadata supplies schema facts; the SQL statement supplies ALTER lifecycle.

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

fn dbt() -> RelationCatalog {
    let manifest = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                    "dbt_version":"1.11.8","adapter_type":"duckdb"},
        "nodes": {
            "model.task158.probe": {
                "unique_id":"model.task158.probe","resource_type":"model",
                "name":"probe","relation_name":"task158_probe",
                "original_file_path":"models/probe.sql","language":"sql",
                "raw_code":"SELECT a FROM t","compiled_code":"SELECT a FROM t",
                "depends_on":{"nodes":["source.task158.t"]}
            }
        },
        "sources":{
            "source.task158.t":{
                "unique_id":"source.task158.t","resource_type":"source",
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
            "source.task158.t":{
                "unique_id":"source.task158.t",
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
    let analyzed = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(analyzed.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;

    let contract = "apiVersion: v3.2.0\nkind: DataContract\nid: task158\nname: TASK-158 parity\nversion: 1.0.0\nstatus: active\nschema:\n  - name: t\n    physicalName: t\n    properties:\n      - name: a\n        physicalType: INTEGER\n      - name: b\n        physicalType: INTEGER\n";
    let evidence =
        parse_odcs_yaml(contract, "duckdb", &RelationCatalog::new(&["t"]).unwrap()).unwrap();
    RelationCatalog::from_schemas(evidence.source_schemas()).unwrap()
}

fn replay(catalog: &RelationCatalog, alter: &str) -> (String, String) {
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = [
        SqlInput::inline("CREATE TABLE t (a INTEGER, b INTEGER)"),
        SqlInput::inline("INSERT INTO t (a, b) VALUES (1, NULL), (1, NULL)"),
        SqlInput::inline(alter),
    ];
    let ids = ["create", "insert", "alter"];
    let configured = sql
        .iter()
        .zip(ids)
        .map(|(sql, id)| ConfiguredSqlInput::new(id, sql, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    let analyzed = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    let last = analyzed
        .inputs()
        .last()
        .unwrap()
        .statements()
        .first()
        .unwrap();
    match last {
        sql_semantic_protocol::ProtocolStatement::Query(query) => {
            assert_eq!(query.write().unwrap().kind(), WriteKind::SchemaAlter);
        }
        other => panic!("ALTER must normalize in every equivalent evidence path: {other:?}"),
    };
    let result = physical_ordered_write_state_plan(&analyzed, &[]);
    assert!(
        matches!(
            result,
            PhysicalWriteStatePlan::Feasible { .. } | PhysicalWriteStatePlan::Residual { .. }
        ),
        "no adapter can invent impossible lifecycle facts: {result:?}"
    );
    match result {
        PhysicalWriteStatePlan::Feasible {
            transitions,
            final_states,
        } => {
            let alteration = transitions.last().unwrap();
            let schema = alteration.after_schema().unwrap();
            (
                format!("{:?}", schema.columns()),
                format!("{:?}", final_states[0].rows()),
            )
        }
        PhysicalWriteStatePlan::Residual { gap, reason, .. } => {
            (format!("Residual:{gap:?}"), reason)
        }
        PhysicalWriteStatePlan::Impossible { reason, .. } => {
            ("Impossible".into(), reason)
        }
    }
}

#[test]
fn direct_and_dbt_equivalent_schema_evidence_preserve_alter_classification() {
    for sql in [
        "ALTER TABLE t ADD COLUMN c INTEGER DEFAULT 2",
        "ALTER TABLE t ADD COLUMN c INTEGER",
        "ALTER TABLE t ADD COLUMN c INTEGER DEFAULT NULL",
        "ALTER TABLE t ADD COLUMN a INTEGER",
        "ALTER TABLE t DROP COLUMN absent",
        "ALTER TABLE t RENAME COLUMN a TO c",
        "ALTER TABLE t DROP COLUMN b",
        "ALTER TABLE t ALTER COLUMN a SET DATA TYPE VARCHAR",
    ] {
        assert_eq!(replay(&direct(), sql), replay(&dbt(), sql), "{sql}");
    }
}

#[cfg(feature = "odcs")]
#[test]
fn direct_and_odcs_equivalent_schema_evidence_preserve_alter_classification() {
    for sql in [
        "ALTER TABLE t ADD COLUMN c INTEGER DEFAULT 2",
        "ALTER TABLE t ADD COLUMN c INTEGER",
        "ALTER TABLE t ADD COLUMN c INTEGER DEFAULT NULL",
        "ALTER TABLE t ADD COLUMN a INTEGER",
        "ALTER TABLE t DROP COLUMN absent",
        "ALTER TABLE t RENAME COLUMN a TO c",
        "ALTER TABLE t DROP COLUMN b",
        "ALTER TABLE t ALTER COLUMN a SET DATA TYPE VARCHAR",
    ] {
        assert_eq!(replay(&direct(), sql), replay(&odcs(), sql), "{sql}");
    }
}
