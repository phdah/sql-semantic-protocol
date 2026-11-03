//! TASK-166: equivalent SQL, compiled dbt and ODCS schema facts must
//! preserve shared-source joint goal feasibility without creating a second bag.

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_joint_distribution_plan, AnalysisBundle,
    ConfiguredSqlInput, ConstraintValue, OutcomeGoal, OutputDistribution, OutputValueCount,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput, WitnessDirection,
    WitnessObligation, OutcomeWitness,
};

const SOURCE: &str = "warehouse.raw.s";
const WHOLE: &str = "SELECT k AS whole FROM warehouse.raw.s";
const FILTERED: &str = "SELECT k AS filtered FROM warehouse.raw.s WHERE k = 1";

fn catalog() -> RelationCatalog {
    RelationCatalog::from_schemas(&[
        RelationSchema::new(
            SOURCE,
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "postgresql").unwrap()],
        ).unwrap(),
    ]).unwrap()
}

fn direct_with_catalog(catalog: &RelationCatalog) -> AnalysisBundle {
    let parser = dialect_from_name("postgresql").unwrap();
    let sql = [SqlInput::inline(WHOLE), SqlInput::inline(FILTERED)];
    let configured = [
        ConfiguredSqlInput::new("model.demo.whole", &sql[0], "postgresql", parser.as_ref()),
        ConfiguredSqlInput::new("model.demo.filtered", &sql[1], "postgresql", parser.as_ref()),
    ];
    analyze_configured_inputs_with_catalog(&configured, catalog).unwrap()
}

fn compiled_dbt() -> AnalysisBundle {
    let manifest = json!({
        "metadata":{
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8", "adapter_type":"postgres"
        },
        "nodes":{
            "model.demo.whole":{
                "unique_id":"model.demo.whole", "resource_type":"model",
                "relation_name":"warehouse.analytics.whole",
                "database":"warehouse", "schema":"analytics",
                "original_file_path":"models/whole.sql", "language":"sql",
                "raw_code":WHOLE, "compiled_code":WHOLE,
                "depends_on":{"nodes":["source.demo.s"]}
            },
            "model.demo.filtered":{
                "unique_id":"model.demo.filtered", "resource_type":"model",
                "relation_name":"warehouse.analytics.filtered",
                "database":"warehouse", "schema":"analytics",
                "original_file_path":"models/filtered.sql", "language":"sql",
                "raw_code":FILTERED, "compiled_code":FILTERED,
                "depends_on":{"nodes":["source.demo.s"]}
            }
        },
        "sources":{
            "source.demo.s":{
                "unique_id":"source.demo.s", "resource_type":"source",
                "relation_name":SOURCE, "database":"warehouse", "schema":"raw",
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
                "metadata":{"type":"BASE TABLE", "database":"warehouse",
                    "schema":"raw", "name":"s"},
                "columns":{"k":{"name":"k","type":"INTEGER","index":1}},
                "stats":{}
            }
        },
        "errors":null
    });
    let manifest = parse_dbt_manifest(&manifest.to_string()).unwrap();
    let catalog = parse_dbt_catalog(&catalog.to_string()).unwrap();
    let parser = dialect_from_name("postgresql").unwrap();
    analyze_dbt_artifacts(&manifest, &catalog, "postgresql", parser.as_ref()).unwrap()
}

#[cfg(feature = "odcs")]
fn odcs_catalog() -> RelationCatalog {
    use sql_semantic_protocol::parse_odcs_yaml;
    let contract = r#"
apiVersion: v3.2.0
kind: DataContract
id: task166sharedsource
name: TASK-166 shared terminal source
version: 1.0.0
status: active
schema:
  - name: s
    physicalName: warehouse.raw.s
    properties:
      - name: k
        physicalType: INTEGER
"#;
    let known = RelationCatalog::new(&[SOURCE]).unwrap();
    let document = parse_odcs_yaml(contract, "postgresql", &known).unwrap();
    RelationCatalog::from_schemas(document.source_schemas()).unwrap()
}

fn proof(bundle: &AnalysisBundle, requested_filtered: u64) -> WitnessDirection {
    let whole = bundle.layers().iter()
        .find(|layer| layer.input_id() == "model.demo.whole").unwrap().id();
    let filtered = bundle.layers().iter()
        .find(|layer| layer.input_id() == "model.demo.filtered").unwrap().id();
    let anchor = OutputDistribution::new("whole", vec![
        OutputValueCount::new(ConstraintValue::Integer(1), 2),
        OutputValueCount::new(ConstraintValue::Integer(2), 1),
    ]).unwrap();
    let targets = [
        OutcomeGoal::new(whole, Some(3), None, vec![anchor]).unwrap(),
        OutcomeGoal::new(filtered, Some(requested_filtered), None, vec![]).unwrap(),
    ];
    physical_joint_distribution_plan(bundle, &targets)
}

fn signature(direction: &WitnessDirection) -> Option<(String, u64, Vec<OutputValueCount>, Vec<u64>)> {
    let WitnessDirection::Feasible(cases) = direction else { return None };
    let [case] = cases.as_slice() else { return None };
    let mut source = None;
    let mut output_counts = Vec::new();
    for obligation in case.obligations() {
        match obligation {
            WitnessObligation::OperatorPopulation {
                witness: OutcomeWitness::SourceRows { relation, rows, columns }
            } if columns.len() == 1 => {
                source = Some((relation.clone(), *rows, columns[0].values().to_vec()));
            }
            WitnessObligation::OutputRows { bounds, .. } => {
                output_counts.push(bounds.minimum());
            }
            _ => {}
        }
    }
    output_counts.sort();
    source.map(|(relation, rows, buckets)| (relation, rows, buckets, output_counts))
}

#[test]
fn full_shared_source_goals_remain_equivalent_across_sql_dbt_and_odcs_metadata() {
    let direct = direct_with_catalog(&catalog());
    let dbt = compiled_dbt();
    for requested in [2, 4] {
        let sql = proof(&direct, requested);
        let compiled = proof(&dbt, requested);
        if requested == 2 {
            let expected = signature(&sql).expect("direct feasible typed source");
            assert_eq!(expected.0, SOURCE);
            assert_eq!(expected.1, 3);
            assert_eq!(expected.3, vec![2, 3]);
            assert_eq!(Some(expected), signature(&compiled), "compiled dbt facts");
        } else {
            assert_eq!(sql, WitnessDirection::Impossible);
            assert_eq!(compiled, WitnessDirection::Impossible);
        }
        #[cfg(feature = "odcs")]
        {
            let odcs = proof(&direct_with_catalog(&odcs_catalog()), requested);
            assert_eq!(signature(&sql), signature(&odcs), "ODCS typed source");
            if requested == 4 {
                assert_eq!(odcs, WitnessDirection::Impossible);
            }
        }
    }
}
