//! Full TASK-143 physical proof, checked against independent frozen bags.
//! These assertions consume the public canonical API, not SQL AST or DuckDB rows.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn literal(value: &Value) -> LiteralValue {
    match value.as_i64() {
        Some(n) => LiteralValue::Number(n.to_string()),
        None if value.is_null() => LiteralValue::Null,
        _ => panic!("unfrozen CTE scalar type: {value}"),
    }
}

fn expected_row(row: &Value, columns: &[String]) -> BTreeMap<String, LiteralValue> {
    columns
        .iter()
        .map(|name| (name.clone(), literal(&row[name])))
        .collect()
}

fn case_bundle(
    sql: &str,
    input: &Value,
) -> (
    sql_semantic_protocol::AnalysisBundle,
    Vec<PhysicalWriteState>,
) {
    case_bundle_with_dialect(sql, input, "duckdb")
}

fn case_bundle_with_dialect(
    sql: &str,
    input: &Value,
    dialect_name: &str,
) -> (
    sql_semantic_protocol::AnalysisBundle,
    Vec<PhysicalWriteState>,
) {
    let dialect = dialect_from_name(dialect_name).unwrap();
    let mut schemas = Vec::new();
    let mut snapshots = Vec::new();
    for (name, rows) in input.as_object().unwrap() {
        schemas.push(
            RelationSchema::new(
                name.clone(),
                vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
            )
            .unwrap(),
        );
        let rows = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), literal(&row["k"]))]))
                    .unwrap()
            })
            .collect();
        snapshots.push(PhysicalWriteState::new(name.clone(), rows).unwrap());
    }
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let input = SqlInput::inline(sql);
    let configured = [ConfiguredSqlInput::new(
        "cte",
        &input,
        dialect_name,
        dialect.as_ref(),
    )];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    (bundle, snapshots)
}

#[test]
fn every_mandatory_frozen_read_case_proves_complete_source_producer_and_terminal_bags() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-143-acceptance-freeze.json")).unwrap();
    let mut proved=Vec::new();
    for case in freeze["cases"].as_array().unwrap() {
        if case["classification"] != "mandatory_exact" || !case["sql"].is_string() || case["id"].as_str().unwrap().contains("WRITE") {
            continue;
        }
        let id=case["id"].as_str().unwrap();
        let (bundle,snapshots)=case_bundle(case["sql"].as_str().unwrap(),&case["initial_bags"]);
        let target=bundle.layers().last().unwrap();
        let PhysicalCteProof::Exact(exact)=
            physical_finite_cte_bag_plan(&bundle,target.id(),&snapshots,None)
        else {panic!("{id}: mandatory source-derived proof must be exact")};
        assert_eq!(exact.sources,vec!["raw"]);
        assert_eq!(exact.nodes.len(),exact.sources.len()+exact.stage_bags.len());
        let mut known=BTreeSet::new();
        for node in &exact.nodes {
            assert!(known.insert(node.id.clone()),"{id}: unique graph node identity");
            for source in &node.inputs {
                assert!(known.contains(&source.reference),"{id}: producer-first graph");
            }
        }
        let source=&case["initial_bags"]["raw"];
        assert_eq!(exact.source_bags["raw"].len(),source.as_array().unwrap().len());
        for (index,row) in source.as_array().unwrap().iter().enumerate() {
            assert_eq!(exact.source_bags["raw"][index].values["k"],literal(&row["k"]));
            assert_eq!(exact.source_bags["raw"][index].source_row_ids,
                vec![row["id"].as_str().unwrap()]);
        }
        let producer=&case["producer_bags"]["stage"];
        let observed_stage=&exact.stage_bags["terminal/cte:0:stage"];
        assert_eq!(observed_stage.len(),producer.as_array().unwrap().len());
        for (index,row) in producer.as_array().unwrap().iter().enumerate() {
            let observed=&observed_stage[index];
            for name in ["k","shifted"] {
                assert_eq!(observed.values[name],literal(&row[name]),"{id}/{name}: stage output");
            }
            assert_eq!(observed.source_row_ids,
                row["source_ids"].as_array().unwrap()
                    .iter().map(|id|id.as_str().unwrap().to_string()).collect::<Vec<_>>());
        }
        let final_names=case["terminal_domains"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        let mut actual=exact.terminal.iter().map(|row|(row.values.clone(),row.source_row_ids.clone())).collect::<Vec<_>>();
        let mut expected=case["terminal_bag"].as_array().unwrap().iter().map(|row|{
            let ids=row["source_ids"].as_array().unwrap().iter().map(|id|id.as_str().unwrap().to_string()).collect::<Vec<_>>();
            (expected_row(row,&final_names),ids)
        }).collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(actual,expected,"{id}: entire terminal bag including duplicate occurrence IDs");
        for (name,domain) in case["terminal_domains"].as_object().unwrap() {
            let observed=&exact.terminal_domains[name];
            assert_eq!(observed.null_allowed,domain["nullable"].as_bool().unwrap());
            assert_eq!(observed.integer_bounds,Some((
                i128::from(domain["lower"]["value"].as_i64().unwrap()),
                i128::from(domain["upper"]["value"].as_i64().unwrap())
            )),"{id}/{name}: exact bounds");
            assert_eq!(domain["lower"]["inclusive"],true);
            assert_eq!(domain["upper"]["inclusive"],true);
        }
        let wanted=expected_row(&case["terminal_bag"][0],&final_names);
        assert!(matches!(
            physical_finite_cte_bag_plan(&bundle,target.id(),&snapshots,Some(&wanted)),
            PhysicalCteProof::Exact(_)
        ),"{id}: positive membership");
        let negative=final_names.into_iter().map(|name|{
            let absent=if name=="total"{3}else{11};
            (name,LiteralValue::Number(absent.to_string()))
        }).collect();
        assert!(matches!(
            physical_finite_cte_bag_plan(&bundle,target.id(),&snapshots,Some(&negative)),
            PhysicalCteProof::Impossible { .. }
        ),"{id}: negative membership from the same complete bag");
        assert_eq!(exact.false_pairs["terminal/cte:0:stage"],
            vec![vec!["raw#0".to_string()]],"{id}: FALSE rejection");
        let null_case=id.contains("NULL-DUP");
        assert_eq!(exact.unknown_pairs["terminal/cte:0:stage"].len(),
            usize::from(null_case),"{id}: UNKNOWN rejection");
        proved.push(id.to_string());
    }
    proved.sort();
    assert_eq!(proved,["78-C143-N-NULL-DUP","78-C143-P-COUNT","78-C143-P-JOIN"]);
}
