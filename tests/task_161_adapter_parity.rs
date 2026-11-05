//! TASK-161: direct SQL, dbt artifact and ODCS source facts feed the
//! same canonical sequential proof. Neither metadata adapter executes SQL
//! or upgrades a parser boundary into an engine/session certification.

use serde_json::{json, Map, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_sequential_state_graph_plan,
    ConfiguredSqlInput, PhysicalSequentialStateGraphProof, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput,
};

fn freeze() -> Value {
    serde_json::from_str(include_str!("../docs/task-161-acceptance-freeze.json")).unwrap()
}

fn expected_columns(case: &Value) -> Vec<(&'static str, Vec<&'static str>)> {
    match case["id"].as_str().unwrap() {
        "84-C161-P-merge" => vec![("t", vec!["id", "v"]), ("s", vec!["id", "v"])],
        "84-C161-P-dml" => vec![("t", vec!["id", "v"])],
        "84-C161-P-ddl" | "84-C161-N-duplicate-column" | "84-C161-N-dropped-read" => {
            vec![("t", vec!["a"])]
        }
        other => panic!("not an independently executable parity case: {other}"),
    }
}

fn sql_catalog(relations: &[(&str, Vec<&str>)]) -> RelationCatalog {
    RelationCatalog::from_schemas(
        &relations
            .iter()
            .map(|(name, columns)| {
                RelationSchema::new(
                    *name,
                    columns
                        .iter()
                        .map(|column| {
                            SchemaColumn::from_sql_type(*column, "INTEGER", "duckdb").unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn dbt_catalog(relations: &[(&str, Vec<&str>)]) -> RelationCatalog {
    let mut sources = Map::new();
    let mut source_catalog = Map::new();
    let mut deps = Vec::new();
    for (relation, columns) in relations {
        let id = format!("source.task161.{relation}");
        deps.push(id.clone());
        let defined = columns
            .iter()
            .map(|column| {
                (
                    (*column).to_string(),
                    json!({
                        "name": column, "data_type": "INTEGER"
                    }),
                )
            })
            .collect::<Map<String, Value>>();
        sources.insert(
            id.clone(),
            json!({
                "unique_id": id, "resource_type": "source",
                "name": relation, "relation_name": relation, "columns": defined
            }),
        );
        let introspected = columns
            .iter()
            .enumerate()
            .map(|(position, column)| {
                (
                    (*column).to_string(),
                    json!({
                        "name":column, "type":"INTEGER", "index":position + 1
                    }),
                )
            })
            .collect::<Map<String, Value>>();
        source_catalog.insert(
            id.clone(),
            json!({
                "unique_id":id, "metadata":{
                    "type":"BASE TABLE","database":"main","schema":"main","name":relation
                },
                "columns":introspected,"stats":{}
            }),
        );
    }
    // dbt dependencies must be represented in compiled model SQL; adding
    // an unused source to the manifest is invalid schema evidence.
    let compiled_sql = if relations.len() == 2 {
        "SELECT t.id FROM t JOIN s ON t.id = s.id".to_string()
    } else {
        format!("SELECT {} FROM t", relations[0].1[0])
    };
    let manifest = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                    "dbt_version":"1.11.8","adapter_type":"duckdb"},
        "nodes":{"model.task161.probe":{
            "unique_id":"model.task161.probe", "resource_type":"model",
            "name":"probe","relation_name":"task161_probe",
            "original_file_path":"models/probe.sql","language":"sql",
            "raw_code":compiled_sql, "compiled_code":compiled_sql,
            "depends_on":{"nodes":deps}
        }},
        "sources":sources
    });
    let catalog = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
                    "dbt_version":"1.11.8"},
        "nodes":{},"sources":source_catalog,"errors":null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let analyzed = analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).unwrap();
    RelationCatalog::from_schemas(analyzed.source_schemas()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog(relations: &[(&str, Vec<&str>)]) -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let mut body = "apiVersion: v3.2.0\nkind: DataContract\nid: task161\nname: TASK-161 parity\nversion: 1.0.0\nstatus: active\nschema:\n".to_string();
    for (relation, columns) in relations {
        body.push_str(&format!(
            "  - name: {relation}\n    physicalName: {relation}\n    properties:\n"
        ));
        for column in columns {
            body.push_str(&format!(
                "      - name: {column}\n        physicalType: INTEGER\n"
            ));
        }
    }
    let names = relations.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    let bundle = parse_odcs_yaml(&body, "duckdb", &RelationCatalog::new(&names).unwrap()).unwrap();
    RelationCatalog::from_schemas(bundle.source_schemas()).unwrap()
}

fn graph(catalog: &RelationCatalog, sql: &[&str]) -> PhysicalSequentialStateGraphProof {
    let parser = dialect_from_name("duckdb").unwrap();
    let inputs = sql
        .iter()
        .map(|statement| SqlInput::inline(*statement))
        .collect::<Vec<_>>();
    let ids = (0..inputs.len())
        .map(|n| format!("step-{n}"))
        .collect::<Vec<_>>();
    let configured = inputs
        .iter()
        .zip(&ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "duckdb", parser.as_ref()))
        .collect::<Vec<_>>();
    let bundle = analyze_configured_inputs_with_catalog(&configured, catalog).unwrap();
    physical_sequential_state_graph_plan(&bundle, &[])
}

// Adapter provenance (DbtCatalog, ODCS or direct SQL) is intentionally
// distinct evidence metadata, not a semantic difference. Compare the typed
// catalog, physical rows and graph obligations without stripping real facts.
fn semantic_proof(proof: &PhysicalSequentialStateGraphProof) -> String {
    match proof {
        PhysicalSequentialStateGraphProof::Exact(exact) => {
            let nodes = exact
                .nodes
                .iter()
                .map(|node| {
                    let schemas = |catalog: &sql_semantic_protocol::PhysicalScriptCatalog| {
                        catalog
                            .schemas
                            .iter()
                            .map(|schema| {
                                (
                                    schema.relation().to_string(),
                                    schema
                                        .columns()
                                        .iter()
                                        .map(|column| {
                                            (
                                                column.name().to_string(),
                                                format!("{:?}", column.data_type()),
                                            )
                                        })
                                        .collect::<Vec<_>>(),
                                )
                            })
                            .collect::<Vec<_>>()
                    };
                    let write = node.write.as_ref().map(|write| {
                        (
                            write.kind(),
                            write.before(),
                            write.after(),
                            write.affected(),
                            write.affected_target_ordinals(),
                            write.definition_source_row_ids(),
                        )
                    });
                    let read = node.read.as_ref().map(|read| {
                        (
                            &read.source,
                            &read.output,
                            read.exact_domains(),
                            &read.surviving_source_ordinals,
                            &read.rejected_source_ordinals,
                        )
                    });
                    (
                        &node.layer_id,
                        &node.before.states,
                        &node.after.states,
                        schemas(&node.before),
                        schemas(&node.after),
                        node.before.exact_domains(),
                        node.after.exact_domains(),
                        write,
                        read,
                    )
                })
                .collect::<Vec<_>>();
            format!("Exact:{:?}", (nodes, &exact.edges, &exact.final_states))
        }
        PhysicalSequentialStateGraphProof::Impossible { reason } => format!("Impossible:{reason}"),
        PhysicalSequentialStateGraphProof::Residual { reason } => format!("Residual:{reason}"),
    }
}

#[test]
fn all_representable_frozen_cases_preserve_direct_dbt_and_odcs_proofs() {
    let freeze = freeze();
    let mut verified = 0;
    for case in freeze["cases"].as_array().unwrap().iter().take(5) {
        let columns = expected_columns(case);
        let sql = case["program"]
            .as_array()
            .unwrap()
            .iter()
            .map(|statement| statement.as_str().unwrap())
            .collect::<Vec<_>>();
        let expected = graph(&sql_catalog(&columns), &sql);
        match case["classification"].as_str().unwrap() {
            "Exact" => assert!(
                matches!(expected, PhysicalSequentialStateGraphProof::Exact(_)),
                "{}",
                case["id"]
            ),
            "Impossible" => assert!(
                matches!(
                    expected,
                    PhysicalSequentialStateGraphProof::Impossible { .. }
                ),
                "{}",
                case["id"]
            ),
            other => panic!("unclassified mandatory parity contract: {other}"),
        }
        assert_eq!(
            semantic_proof(&expected),
            semantic_proof(&graph(&dbt_catalog(&columns), &sql)),
            "dbt: {}",
            case["id"]
        );
        #[cfg(feature = "odcs")]
        assert_eq!(
            semantic_proof(&expected),
            semantic_proof(&graph(&odcs_catalog(&columns), &sql)),
            "ODCS: {}",
            case["id"]
        );
        verified += 1;
    }
    assert_eq!(verified, 5);
}

#[test]
fn missing_physical_evidence_remains_residual_across_equivalent_sources() {
    let expected = graph(&sql_catalog(&[("t", vec!["a"])]), &["SELECT a FROM t"]);
    assert!(matches!(
        expected,
        PhysicalSequentialStateGraphProof::Residual { .. }
    ));
    assert_eq!(
        semantic_proof(&expected),
        semantic_proof(&graph(
            &dbt_catalog(&[("t", vec!["a"])]),
            &["SELECT a FROM t"]
        ))
    );
    #[cfg(feature = "odcs")]
    assert_eq!(
        semantic_proof(&expected),
        semantic_proof(&graph(
            &odcs_catalog(&[("t", vec!["a"])]),
            &["SELECT a FROM t"]
        ))
    );
}

#[test]
fn adapters_are_metadata_evidence_only_not_transaction_or_ddl_executors() {
    let freeze = freeze();
    assert_eq!(freeze["adapter_cells"].as_array().unwrap().len(), 21);
    assert_eq!(freeze["conditional_engine_owner"], "TASK-162");
    let dialect = dialect_from_name("duckdb").unwrap();
    let manifest = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
                    "dbt_version":"1.11.8","adapter_type":"duckdb"},
        "nodes":{"model.task161.probe":{
            "unique_id":"model.task161.probe","resource_type":"model",
            "name":"probe","relation_name":"task161_probe",
            "original_file_path":"models/probe.sql","language":"sql",
            "raw_code":"DROP TABLE t", "compiled_code":"DROP TABLE t",
            "depends_on":{"nodes":[]}
        }},
        "sources":{}
    });
    let catalog = json!({
        "metadata":{"dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
                    "dbt_version":"1.11.8"},
        "nodes":{},"sources":{},"errors":null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    assert!(
        analyze_dbt_artifacts(&manifest, &catalog, "duckdb", dialect.as_ref()).is_err(),
        "dbt models must not be mistaken for independent DDL execution"
    );
}
