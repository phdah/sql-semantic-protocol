//! TASK-165: same typed complete-histogram proof with SQL, compiled dbt and ODCS metadata.

use serde_json::{json, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, to_bundle_json, AnalysisBundle, ConfiguredSqlInput,
    ConstraintValue, OutcomeGoal, OutcomeGoalStatus, OutputDistribution, OutputValueCount,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

const RELATION: &str = "warehouse.raw.s";
const SQL: &str = "SELECT k FROM warehouse.raw.s";

fn source_schema() -> RelationCatalog {
    RelationCatalog::from_schemas(&[RelationSchema::new(
        RELATION,
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").expect("source INTEGER")],
    )
    .expect("schema")])
    .expect("catalog")
}

fn sql_bundle(catalog: &RelationCatalog) -> AnalysisBundle {
    let parser = dialect_from_name("postgresql").expect("dialect");
    let sql = SqlInput::inline(SQL);
    let input = ConfiguredSqlInput::new("task-165", &sql, "postgresql", parser.as_ref());
    analyze_configured_inputs_with_catalog(&[input], catalog).expect("direct SQL")
}

fn dbt_bundle() -> AnalysisBundle {
    let artifact = json!({
        "metadata":{
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8", "adapter_type":"postgres"
        },
        "nodes":{
            "model.demo.task165":{
                "unique_id":"model.demo.task165", "resource_type":"model",
                "relation_name":"warehouse.analytics.task165",
                "database":"warehouse", "schema":"analytics",
                "original_file_path":"models/task165.sql", "language":"sql",
                "raw_code":SQL, "compiled_code":SQL,
                "depends_on":{"nodes":["source.demo.s"]}
            }
        },
        "sources":{
            "source.demo.s":{
                "unique_id":"source.demo.s", "resource_type":"source",
                "relation_name":RELATION, "database":"warehouse", "schema":"raw",
                "name":"s", "columns":{"k":{"name":"k","data_type":"INTEGER"}}
            }
        }
    });
    let catalog = json!({
        "metadata":{
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version":"1.11.8"
        },
        "nodes":{},
        "sources":{
            "source.demo.s":{
                "unique_id":"source.demo.s",
                "metadata":{
                    "type":"BASE TABLE", "database":"warehouse",
                    "schema":"raw", "name":"s"
                },
                "columns":{"k":{"name":"k","type":"INTEGER","index":1}},
                "stats":{}
            }
        },
        "errors":null
    });
    let manifest = parse_dbt_manifest(&artifact.to_string()).expect("compiled artifact");
    let catalog = parse_dbt_catalog(&catalog.to_string()).expect("warehouse catalog");
    let parser = dialect_from_name("postgresql").expect("dialect");
    analyze_dbt_artifacts(&manifest, &catalog, "postgresql", parser.as_ref())
        .expect("compiled dbt plus authoritative warehouse types")
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;

    let contract = r#"
apiVersion: v3.2.0
kind: DataContract
id: task165output
name: TASK-165 output source
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: k
        physicalType: INTEGER
"#;
    let known = RelationCatalog::new(&[RELATION]).expect("known relation");
    let document = parse_odcs_yaml(contract, "postgresql", &known).expect("ODCS schema");
    RelationCatalog::from_schemas(document.source_schemas()).expect("ODCS catalog")
}

fn evaluated(
    mut bundle: AnalysisBundle,
    rows: u64,
    entries: &[(ConstraintValue, u64)],
) -> (OutcomeGoalStatus, Option<String>, Value) {
    let layer = bundle
        .layers()
        .last()
        .expect("output layer")
        .id()
        .to_string();
    let histogram = OutputDistribution::new(
        "k",
        entries
            .iter()
            .map(|(value, count)| OutputValueCount::new(value.clone(), *count))
            .collect(),
    )
    .expect("histogram");
    bundle
        .set_outcome_goals(&[
            OutcomeGoal::new(layer, Some(rows), None, vec![histogram]).expect("complete target")
        ])
        .expect("attach goals");
    let goal = &bundle.outcome_goals()[0];
    let emission: Value = serde_json::from_str(&to_bundle_json(&bundle)).expect("protocol");
    let witness = emission["outcome_goals"][0]["witness"]["kind"]
        .as_str()
        .map(str::to_string);
    (
        goal.status(),
        witness,
        emission["outcome_goals"][0]["assessment"]["status"].clone(),
    )
}

#[test]
fn sql_dbt_odcs_typed_evidence_equivalence_preserves_complete_goal_proofs() {
    let distribution = [(ConstraintValue::Integer(1), 2), (ConstraintValue::Null, 1)];
    let compare = |rows| {
        let direct = evaluated(sql_bundle(&source_schema()), rows, &distribution);
        let dbt = evaluated(dbt_bundle(), rows, &distribution);
        assert_eq!(direct, dbt, "compiled dbt / direct source evidence");
        #[cfg(feature = "odcs")]
        assert_eq!(
            direct,
            evaluated(sql_bundle(&odcs_catalog()), rows, &distribution),
            "ODCS schema evidence"
        );
        direct
    };

    let exact = compare(3);
    assert_eq!(exact.0, OutcomeGoalStatus::Feasible);
    assert_eq!(exact.1.as_deref(), Some("source_rows"));
    assert_eq!(exact.2, "feasible");

    let impossible = compare(2);
    assert_eq!(impossible.0, OutcomeGoalStatus::Unsatisfiable);
    assert!(impossible.1.is_none());
    assert_eq!(impossible.2, "unsatisfiable");
}
