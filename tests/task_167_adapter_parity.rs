//! TASK-167: direct SQL, compiled dbt and ODCS supply equivalent canonical
//! facts for both terminals. One physical source assignment is never cloned.

use serde_json::json;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_artifacts, dialect_from_name,
    parse_dbt_catalog, parse_dbt_manifest, physical_joint_terminal_bag_plan, AnalysisBundle,
    ConfiguredSqlInput, ConstraintValue, LiteralValue, OutcomeGoal, OutputDistribution,
    OutputValueCount, PhysicalJointOtherExact, PhysicalJointTerminalProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use std::collections::BTreeMap;

const LEFT: &str = "warehouse.raw.l";
const RIGHT: &str = "warehouse.raw.r";
const JOINED: &str = "SELECT l.k AS k, COUNT(*) AS n FROM warehouse.raw.l l JOIN warehouse.raw.r r ON l.k = r.k WHERE l.k = 1 GROUP BY l.k HAVING COUNT(*) >= 2";
const WINDOW: &str = "SELECT k, COUNT(*) OVER (PARTITION BY k) AS copies FROM warehouse.raw.l";
const SET: &str = "SELECT k, v FROM warehouse.raw.l UNION ALL SELECT k, v FROM warehouse.raw.r";

fn source_schemas() -> Vec<RelationSchema> {
    [LEFT, RIGHT]
        .into_iter()
        .map(|name| {
            RelationSchema::new(
                name,
                [("id", "VARCHAR"), ("k", "INTEGER"), ("v", "INTEGER")]
                    .into_iter()
                    .map(|(column, kind)| {
                        SchemaColumn::from_sql_type(column, kind, "postgresql").unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect()
}

fn direct(catalog: &RelationCatalog, second: &str) -> AnalysisBundle {
    let parser = dialect_from_name("postgresql").unwrap();
    let sql = [SqlInput::inline(JOINED), SqlInput::inline(second)];
    let configured = [
        ConfiguredSqlInput::new("model.demo.joined", &sql[0], "postgresql", parser.as_ref()),
        ConfiguredSqlInput::new("model.demo.second", &sql[1], "postgresql", parser.as_ref()),
    ];
    analyze_configured_inputs_with_catalog(&configured, catalog).unwrap()
}

fn dbt(second: &str) -> AnalysisBundle {
    let source = |unique_id: &str, name: &str, relation_name: &str| {
        json!({
            "unique_id":unique_id,"resource_type":"source",
            "relation_name":relation_name,"database":"warehouse",
            "schema":"raw","name":name,
            "columns":{
                "id":{"name":"id","data_type":"VARCHAR"},
                "k":{"name":"k","data_type":"INTEGER"},
                "v":{"name":"v","data_type":"INTEGER"}
            }
        })
    };
    let model = |unique_id: &str, name: &str, sql: &str, depends_on: Vec<&str>| {
        json!({
            "unique_id":unique_id,"resource_type":"model",
            "relation_name":format!("warehouse.analytics.{name}"),
            "database":"warehouse","schema":"analytics",
            "original_file_path":format!("models/{name}.sql"),
            "language":"sql","raw_code":sql,"compiled_code":sql,
            "depends_on":{"nodes":depends_on}
        })
    };
    let manifest = json!({
        "metadata":{
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version":"1.11.8","adapter_type":"postgres"
        },
        "nodes":{
            "model.demo.joined":model("model.demo.joined","joined",JOINED,vec!["source.demo.l","source.demo.r"]),
            "model.demo.second":model(
                "model.demo.second",
                "second",
                second,
                if second == SET {vec!["source.demo.l","source.demo.r"]} else {vec!["source.demo.l"]}
            )
        },
        "sources":{
            "source.demo.l":source("source.demo.l","l",LEFT),
            "source.demo.r":source("source.demo.r","r",RIGHT)
        }
    });
    let catalog = json!({
        "metadata":{
            "dbt_schema_version":"https://schemas.getdbt.com/dbt/catalog/v1.json",
            "dbt_version":"1.11.8"
        },
        "nodes":{},
        "sources":{
            "source.demo.l":{
                "unique_id":"source.demo.l",
                "metadata":{"type":"BASE TABLE","database":"warehouse","schema":"raw","name":"l"},
                "columns":{
                    "id":{"name":"id","type":"VARCHAR","index":1},
                    "k":{"name":"k","type":"INTEGER","index":2},
                    "v":{"name":"v","type":"INTEGER","index":3}
                },
                "stats":{}
            },
            "source.demo.r":{
                "unique_id":"source.demo.r",
                "metadata":{"type":"BASE TABLE","database":"warehouse","schema":"raw","name":"r"},
                "columns":{
                    "id":{"name":"id","type":"VARCHAR","index":1},
                    "k":{"name":"k","type":"INTEGER","index":2},
                    "v":{"name":"v","type":"INTEGER","index":3}
                },
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
id: task167sources
name: TASK-167 shared physical inputs
version: 1.0.0
status: active
schema:
  - name: l
    physicalName: warehouse.raw.l
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: v
        physicalType: INTEGER
  - name: r
    physicalName: warehouse.raw.r
    properties:
      - name: id
        physicalType: VARCHAR
      - name: k
        physicalType: INTEGER
      - name: v
        physicalType: INTEGER
"#;
    let known = RelationCatalog::new(&[LEFT, RIGHT]).unwrap();
    let document = parse_odcs_yaml(contract, "postgresql", &known).unwrap();
    RelationCatalog::from_schemas(document.source_schemas()).unwrap()
}

fn snapshots() -> Vec<PhysicalWriteState> {
    let row = |id: &str, k: Option<i64>, v: i64| {
        PhysicalWriteRow::new(BTreeMap::from([
            ("id".into(), LiteralValue::Text(id.into())),
            (
                "k".into(),
                k.map_or(LiteralValue::Null, |value| {
                    LiteralValue::Number(value.to_string())
                }),
            ),
            ("v".into(), LiteralValue::Number(v.to_string())),
        ]))
        .unwrap()
    };
    vec![
        PhysicalWriteState::new(
            LEFT,
            vec![
                row("l0", Some(1), 10),
                row("l1", Some(1), 20),
                row("l2", None, 30),
            ],
        )
        .unwrap(),
        PhysicalWriteState::new(RIGHT, vec![row("r0", Some(1), 10)]).unwrap(),
    ]
}

fn goals(bundle: &AnalysisBundle, secondary_rows: u64, set: bool) -> Vec<OutcomeGoal> {
    let layer = |name: &str| {
        bundle
            .layers()
            .iter()
            .find(|layer| layer.input_id() == name)
            .unwrap()
            .id()
            .to_string()
    };
    let distribution = |column: &str, values: &[(i64, u64)]| {
        OutputDistribution::new(
            column,
            values
                .iter()
                .map(|(value, count)| {
                    OutputValueCount::new(ConstraintValue::Integer(*value), *count)
                })
                .collect(),
        )
        .unwrap()
    };
    vec![
        OutcomeGoal::new(
            layer("model.demo.joined"),
            Some(1),
            Some(1),
            vec![distribution("n", &[(2, 1)])],
        )
        .unwrap(),
        OutcomeGoal::new(
            layer("model.demo.second"),
            Some(secondary_rows),
            None,
            if set {
                if secondary_rows == 5 {
                    vec![distribution("v", &[(10, 3), (20, 1), (30, 1)])]
                } else {
                    vec![distribution("v", &[(10, 2), (20, 1), (30, 1)])]
                }
            } else if secondary_rows == 4 {
                vec![distribution("copies", &[(1, 2), (2, 2)])]
            } else {
                vec![distribution("copies", &[(1, 1), (2, 2)])]
            },
        )
        .unwrap(),
    ]
}

fn normalized(rows: &[BTreeMap<String, LiteralValue>]) -> Vec<String> {
    let mut result = rows
        .iter()
        .map(|row| format!("{row:?}"))
        .collect::<Vec<_>>();
    result.sort();
    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct JointSignature {
    grouped: Vec<String>,
    secondary: Vec<String>,
    sources: Vec<String>,
    rejected_pairs: usize,
}

fn signature(bundle: &AnalysisBundle, requested_rows: u64, set: bool) -> Option<JointSignature> {
    let result = physical_joint_terminal_bag_plan(
        bundle,
        &goals(bundle, requested_rows, set),
        &snapshots(),
    );
    let PhysicalJointTerminalProof::Exact(exact) = result else {
        return None;
    };
    let secondary = match exact.other {
        PhysicalJointOtherExact::Window(window) => normalized(&window.terminal),
        PhysicalJointOtherExact::Set(set) => normalized(&set.terminal),
    };
    Some(JointSignature {
        grouped: normalized(&exact.grouped.terminal),
        secondary,
        sources: exact
            .sources
            .iter()
            .map(|source| source.relation().to_string())
            .collect(),
        rejected_pairs: exact.grouped.rejected.len(),
    })
}

#[test]
fn sql_dbt_odcs_have_identical_joint_bags_and_impossibility_boundaries() {
    for (other, set, rows) in [(WINDOW, false, 3), (SET, true, 4)] {
        let sql_bundle = direct(&RelationCatalog::from_schemas(&source_schemas()).unwrap(), other);
        let dbt = dbt(other);
        let expected = signature(&sql_bundle, rows, set).expect("direct SQL exact pair");
        assert_eq!(expected.sources, vec![LEFT.to_string(), RIGHT.to_string()]);
        assert_eq!(expected.rejected_pairs, 1);
        assert_eq!(signature(&dbt, rows, set), Some(expected.clone()));
        for bundle in [&sql_bundle, &dbt] {
            assert!(matches!(
                physical_joint_terminal_bag_plan(
                    bundle, &goals(bundle, rows + 1, set), &snapshots()
                ),
                PhysicalJointTerminalProof::Impossible { .. }
            ));
        }
        #[cfg(feature = "odcs")]
        {
            let odcs = direct(&odcs_catalog(), other);
            assert_eq!(signature(&odcs, rows, set), Some(expected));
            assert!(matches!(
                physical_joint_terminal_bag_plan(&odcs, &goals(&odcs, rows + 1, set), &snapshots()),
                PhysicalJointTerminalProof::Impossible { .. }
            ));
        }
    }
}
