//! TASK-160: SQL owns statement execution; dbt catalog and ODCS only
//! contribute the same authoritative typed physical source evidence.

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_ordered_lifecycle_plan, ConfiguredSqlInput,
    PhysicalOrderedLifecycleProof, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn direct() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        "t",
        vec![SchemaColumn::from_sql_type("a", "INTEGER", "duckdb").unwrap()],
    )
    .unwrap()])
    .unwrap()
}

fn dbt_bundle(compiled_sql: &str) -> Result<sql_semantic_protocol::AnalysisBundle, String> {
    let manifest = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                    "dbt_version":"1.11.8","adapter_type":"duckdb"},
        "nodes":{
            "model.task160.probe":{
                "unique_id":"model.task160.probe","resource_type":"model",
                "name":"probe","relation_name":"task160_probe",
                "original_file_path":"models/probe.sql","language":"sql",
                "raw_code":compiled_sql,"compiled_code":compiled_sql,
                "depends_on":{"nodes":["source.task160.t"]}
            }
        },
        "sources":{
            "source.task160.t":{
                "unique_id":"source.task160.t","resource_type":"source",
                "relation_name":"t","name":"t",
                "columns":{"a":{"name":"a","data_type":"INTEGER"}}
            }
        }
    });
    let catalog = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
                    "dbt_version":"1.11.8"},
        "nodes":{},"sources":{
            "source.task160.t":{
                "unique_id":"source.task160.t",
                "metadata":{"type":"BASE TABLE","database":"main","schema":"main","name":"t"},
                "columns":{"a":{"name":"a","type":"INTEGER","index":1}},
                "stats":{}
            }
        },
        "errors":null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref())
        .map_err(|error| error.to_string())
}

fn dbt() -> RelationCatalog {
    let bundle = dbt_bundle("SELECT a FROM t").unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;

    let contract = "apiVersion: v3.2.0\nkind: DataContract\nid: task160\nname: TASK-160 parity\nversion: 1.0.0\nstatus: active\nschema:\n  - name: t\n    physicalName: t\n    properties:\n      - name: a\n        physicalType: INTEGER\n";
    let bundle =
        parse_odcs_yaml(contract, "duckdb", &RelationCatalog::new(&["t"]).unwrap()).unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

fn outcome(catalog: &RelationCatalog, program: &[&str]) -> String {
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = program
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let ids = (0..inputs.len())
        .map(|index| format!("step-{index}"))
        .collect::<Vec<_>>();
    let configured = inputs
        .iter()
        .zip(&ids)
        .map(|(sql, id)| ConfiguredSqlInput::new(id, sql, "duckdb", dialect.as_ref()))
        .collect::<Vec<_>>();
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    match physical_ordered_lifecycle_plan(&bundle, &[]) {
        PhysicalOrderedLifecycleProof::Exact(proof) => {
            assert_eq!(proof.reads.len(), 1);
            format!(
                "Exact:{:?}:{:?}:{:?}:{:?}",
                proof
                    .checkpoints
                    .iter()
                    .map(|step| step.states.clone())
                    .collect::<Vec<_>>(),
                proof.reads[0].output.rows(),
                proof.reads[0].exact_domains(),
                proof.final_states
            )
        }
        PhysicalOrderedLifecycleProof::Impossible { reason } => format!("Impossible:{reason}"),
        PhysicalOrderedLifecycleProof::Residual { reason } => format!("Residual:{reason}"),
    }
}

#[test]
fn equivalent_direct_and_dbt_schema_evidence_produce_identical_read_and_catalog() {
    for program in [
        vec![
            "CREATE TABLE t(a INTEGER)",
            "INSERT INTO t(a) VALUES (1), (NULL), (1)",
            "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 2",
            "SELECT a, b FROM t",
            "DROP TABLE t",
        ],
        vec![
            "CREATE TABLE t(a INTEGER)",
            "INSERT INTO t(a) VALUES (1), (NULL), (1), (2)",
            "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 2",
            "SELECT a, b FROM t WHERE a >= 2",
            "DROP TABLE t",
        ],
    ] {
        let expected = outcome(&direct(), &program);
        assert!(expected.starts_with("Exact:"), "{expected}");
        assert_eq!(expected, outcome(&dbt(), &program));
        #[cfg(feature = "odcs")]
        assert_eq!(expected, outcome(&odcs(), &program));
    }
}

#[test]
fn equivalent_schema_evidence_preserves_missing_read_and_duplicate_column_contradictions() {
    for program in [
        vec![
            "CREATE TABLE t(a INTEGER)",
            "INSERT INTO t VALUES (1)",
            "DROP TABLE t",
            "SELECT a FROM t",
        ],
        vec![
            "CREATE TABLE t(a INTEGER)",
            "INSERT INTO t VALUES (1)",
            "ALTER TABLE t ADD COLUMN a INTEGER",
            "SELECT a FROM t",
            "DROP TABLE t",
        ],
    ] {
        let expected = outcome(&direct(), &program);
        assert!(expected.starts_with("Impossible:"), "{expected}");
        assert_eq!(expected, outcome(&dbt(), &program));
        #[cfg(feature = "odcs")]
        assert_eq!(expected, outcome(&odcs(), &program));
    }
}

#[test]
fn dbt_model_ddl_is_not_mistaken_for_a_select_or_an_executable_lifecycle() {
    let error = dbt_bundle("DROP TABLE t").unwrap_err();
    assert!(error.contains("Expected: SELECT"), "{error}");
}
