//! TASK-115: cross-adapter proof parity over the same controlled physical bags.
use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, analyze_dbt_manifest_with_schemas, dialect_from_name,
    parse_dbt_manifest, physical_finite_chain_bag_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalFiniteChainColumnDomain, PhysicalFiniteChainProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn rows(case: &Value) -> Vec<PhysicalWriteState> {
    ["l", "r", "t"]
        .into_iter()
        .map(|relation| {
            let bag = case["physical_source_bags"][relation]
                .as_array()
                .expect("complete source bag")
                .iter()
                .map(|row| {
                    let values = row
                        .as_object()
                        .expect("named columns")
                        .iter()
                        .map(|(name, value)| {
                            let scalar = if value.is_null() {
                                LiteralValue::Null
                            } else if let Some(value) = value.as_i64() {
                                LiteralValue::Number(value.to_string())
                            } else {
                                LiteralValue::Text(value.as_str().expect("text").to_string())
                            };
                            (name.clone(), scalar)
                        })
                        .collect::<BTreeMap<_, _>>();
                    PhysicalWriteRow::new(values).expect("physical row")
                })
                .collect();
            PhysicalWriteState::new(relation, bag).expect("closed physical bag")
        })
        .collect()
}

fn schemas(case: &Value) -> Vec<RelationSchema> {
    ["l", "r", "t"]
        .into_iter()
        .map(|relation| {
            let columns = case["source_column_types"][relation]
                .as_object()
                .expect("frozen source schema")
                .iter()
                .map(|(name, declaration)| {
                    SchemaColumn::from_sql_type(
                        name,
                        declaration
                            .as_str()
                            .expect("SQL column type")
                            .split_whitespace()
                            .next()
                            .expect("SQL base type"),
                        "postgresql",
                    )
                    .expect("physical source type")
                })
                .collect();
            RelationSchema::new(relation, columns).expect("physical relation schema")
        })
        .collect()
}

fn source_manifest(case: &Value) -> Value {
    let mut sources = Map::new();
    for relation in ["l", "r", "t"] {
        let mut columns = Map::new();
        for (column, declaration) in case["source_column_types"][relation]
            .as_object()
            .expect("source types")
        {
            columns.insert(
                column.clone(),
                json!({
                    "name": column,
                    "data_type": declaration.as_str().expect("SQL datatype")
                        .split_whitespace().next().expect("base datatype")
                }),
            );
        }
        sources.insert(
            format!("source.demo.{relation}"),
            json!({
                "unique_id": format!("source.demo.{relation}"),
                "resource_type": "source",
                "relation_name": format!("db.raw.{relation}"),
                "database": "db",
                "schema": "raw",
                "name": relation,
                "columns": columns,
            }),
        );
    }
    Value::Object(sources)
}

fn model(id: &str, relation: &str, sql: &str, dependencies: Vec<String>) -> Value {
    json!({
        "unique_id": id,
        "resource_type": "model",
        "relation_name": format!("db.analytics.{relation}"),
        "database": "db",
        "schema": "analytics",
        "original_file_path": format!("models/{relation}.sql"),
        "language": "sql",
        "raw_code": sql,
        "compiled_code": sql,
        "depends_on": { "nodes": dependencies }
    })
}

fn qualified_dbt_sql(sql: &str) -> String {
    sql.replace("FROM l", "FROM db.raw.l AS l")
        .replace("JOIN r", "JOIN db.raw.r AS r")
        .replace("JOIN t", "JOIN db.raw.t AS t")
        .replace("FROM stage", "FROM db.analytics.stage AS stage")
}

fn dbt(case: &Value) -> sql_semantic_protocol::AnalysisBundle {
    let all_sources = ["l", "r", "t"]
        .into_iter()
        .map(|source| format!("source.demo.{source}"))
        .collect::<Vec<_>>();
    let sql = case["sql"].as_str().expect("immutable SQL");
    let grouped = case["variant"] == "downstream_group_count";
    let mut nodes = Map::new();
    if grouped {
        let statements = sql.split(';').collect::<Vec<_>>();
        assert_eq!(
            statements.len(),
            2,
            "grouped fixture consists of two stages"
        );
        let stage = statements[0]
            .trim()
            .strip_prefix("CREATE TABLE stage AS ")
            .expect("frozen CTAS producer");
        nodes.insert(
            "model.demo.stage".to_string(),
            model(
                "model.demo.stage",
                "stage",
                &qualified_dbt_sql(stage),
                all_sources,
            ),
        );
        nodes.insert(
            "model.demo.task115".to_string(),
            model(
                "model.demo.task115",
                "task115_output",
                &qualified_dbt_sql(statements[1].trim()),
                vec!["model.demo.stage".to_string()],
            ),
        );
    } else {
        nodes.insert(
            "model.demo.task115".to_string(),
            model(
                "model.demo.task115",
                "task115_output",
                &qualified_dbt_sql(sql),
                all_sources,
            ),
        );
    }
    let manifest = json!({
        "metadata": {
            "dbt_schema_version": "https://schemas.getdbt.com/dbt/manifest/v12.json",
            "dbt_version": "1.11.8",
            "adapter_type": "postgres"
        },
        "nodes": nodes,
        "sources": source_manifest(case)
    });
    let parser = dialect_from_name("postgresql").expect("postgres parser");
    let parsed = parse_dbt_manifest(&manifest.to_string()).expect("dbt manifest adapter");
    analyze_dbt_manifest_with_schemas(&parsed, "postgresql", parser.as_ref())
        .expect("canonical dbt source analysis")
}

fn direct(case: &Value, catalog: &RelationCatalog) -> sql_semantic_protocol::AnalysisBundle {
    let sql = case["sql"].as_str().expect("frozen SQL");
    let statements = sql
        .split(';')
        .map(str::trim)
        .filter(|sql| !sql.is_empty())
        .map(SqlInput::inline)
        .collect::<Vec<_>>();
    let parser = dialect_from_name("postgresql").expect("dialect");
    let ids = (0..statements.len())
        .map(|index| format!("task115-{index}"))
        .collect::<Vec<_>>();
    let configured = statements
        .iter()
        .zip(&ids)
        .map(|(input, id)| ConfiguredSqlInput::new(id, input, "postgresql", parser.as_ref()))
        .collect::<Vec<_>>();
    analyze_configured_inputs_with_catalog(&configured, catalog).expect("direct SQL analysis")
}

#[derive(Debug, PartialEq, Eq)]
struct CanonicalBag {
    sources: [String; 3],
    filtered_true_ids: Vec<String>,
    filtered_not_true_ids: Vec<String>,
    selected: Vec<[String; 3]>,
    rejected: Vec<[String; 3]>,
    contributions: BTreeMap<i128, Vec<[String; 3]>>,
    terminal: Vec<BTreeMap<String, LiteralValue>>,
    terminal_domains: BTreeMap<String, PhysicalFiniteChainColumnDomain>,
}

fn certified(case: &Value, bundle: &sql_semantic_protocol::AnalysisBundle) -> CanonicalBag {
    let target = bundle.layers().last().expect("terminal");
    let snapshots = rows(case)
        .into_iter()
        .map(|state| {
            let qualified = format!("db.raw.{}", state.relation());
            if bundle
                .source_schemas()
                .iter()
                .any(|schema| schema.relation() == qualified)
            {
                PhysicalWriteState::new(qualified, state.rows().to_vec())
                    .expect("dbt physical relation identities")
            } else {
                state
            }
        })
        .collect::<Vec<_>>();
    let proof = physical_finite_chain_bag_plan(bundle, target.id(), &snapshots);
    let PhysicalFiniteChainProof::Exact(exact) = proof else {
        panic!("missing adapter physical bag proof: {proof:?}");
    };
    let mut selected = exact.selected.clone();
    let mut rejected = exact.rejected.clone();
    let mut filtered_true_ids = exact.filtered_true_ids.clone();
    let mut filtered_not_true_ids = exact.filtered_not_true_ids.clone();
    selected.sort();
    rejected.sort();
    filtered_true_ids.sort();
    filtered_not_true_ids.sort();
    CanonicalBag {
        sources: exact.sources.clone(),
        filtered_true_ids,
        filtered_not_true_ids,
        selected,
        rejected,
        contributions: exact.contributions.clone(),
        terminal: exact.terminal.clone(),
        terminal_domains: exact.terminal_domains.clone(),
    }
}

#[test]
fn frozen_eight_bags_produce_identical_canonical_proofs_from_sql_and_dbt() {
    let inventory: Value =
        serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
            .expect("frozen matrix");
    for case in inventory["cases"].as_array().expect("eight cases") {
        let catalog = RelationCatalog::from_schemas(&schemas(case)).expect("physical catalog");
        let direct = certified(case, &direct(case, &catalog));
        let dbt = certified(case, &dbt(case));
        assert_eq!(
            dbt, direct,
            "dbt must not change source or terminal identity bags"
        );
    }
}

#[cfg(feature = "odcs")]
#[test]
fn frozen_eight_bags_produce_identical_odcs_source_proofs() {
    use sql_semantic_protocol::parse_odcs_yaml;

    let inventory: Value =
        serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
            .expect("frozen matrix");
    for case in inventory["cases"].as_array().expect("eight cases") {
        let mut contract = String::from(
            "apiVersion: v3.2.0\nkind: DataContract\nid: task115finite\nname: Frozen physical join metadata\nversion: 1.0.0\nstatus: active\nschema:\n",
        );
        for relation in ["l", "r", "t"] {
            contract.push_str(&format!(
                "  - name: {relation}\n    physicalName: {relation}\n    properties:\n"
            ));
            for (column, declaration) in case["source_column_types"][relation]
                .as_object()
                .expect("schema")
            {
                let base = declaration
                    .as_str()
                    .expect("SQL type")
                    .split_whitespace()
                    .next()
                    .expect("base type");
                contract.push_str(&format!(
                    "      - name: {column}\n        physicalType: {base}\n"
                ));
            }
        }
        let names = RelationCatalog::new(&["l", "r", "t"]).expect("physical relation catalog");
        let odcs = parse_odcs_yaml(&contract, "postgresql", &names).expect("ODCS source adapter");
        let odcs_catalog =
            RelationCatalog::from_schemas(odcs.source_schemas()).expect("ODCS schema catalog");
        let direct_catalog = RelationCatalog::from_schemas(&schemas(case)).expect("direct catalog");
        let baseline = certified(case, &direct(case, &direct_catalog));
        let odcs = certified(case, &direct(case, &odcs_catalog));
        assert_eq!(
            odcs, baseline,
            "ODCS must not alter canonical source assignments"
        );
    }
}
