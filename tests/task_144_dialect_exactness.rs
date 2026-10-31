//! TASK-144: parser-accepted SQL dialects must preserve the SAME complete
//! canonical physical bag, not only accept the syntax at parser boundary.

mod common;

use common::DIALECTS;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name,
    physical_finite_cte_bag_plan, physical_finite_view_read_plan,
    physical_ordered_cte_read_plan, ConfiguredSqlInput, LiteralValue,
    PhysicalCteProof, PhysicalOrderedCteProof, PhysicalProducerRow, PhysicalViewProof,
    PhysicalWriteRow, PhysicalWriteState, RelationCatalog, RelationSchema,
    SchemaColumn, SqlInput,
};
use sqlparser::parser::Parser;
use std::collections::BTreeMap;

fn literal(value: &Value) -> LiteralValue {
    value.as_i64().map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string()))
}

fn source_evidence(case: &Value) -> (RelationCatalog, Vec<PhysicalWriteState>) {
    let mut schemas = Vec::new();
    let mut states = Vec::new();
    for (name, bag) in case["initial_bags"].as_object().unwrap() {
        let columns = if name == "target" { vec!["k", "shifted"] } else { vec!["k"] };
        let schema = columns.iter().map(|col| {
            SchemaColumn::from_sql_type(col, "INTEGER", "duckdb").unwrap()
        }).collect();
        schemas.push(RelationSchema::new(name.clone(), schema).unwrap());
        let rows = bag.as_array().unwrap().iter().map(|row| {
            let values = if row.is_object() {
                columns.iter().map(|col| ((*col).to_string(), literal(&row[*col]))).collect()
            } else {
                BTreeMap::from([("k".to_string(), literal(row))])
            };
            PhysicalWriteRow::new(values).unwrap()
        }).collect();
        states.push(PhysicalWriteState::new(name.clone(), rows).unwrap());
    }
    (RelationCatalog::from_schemas(&schemas).unwrap(), states)
}

fn assert_terminal(id: &str, dialect: &str, rows: &[PhysicalProducerRow], case: &Value) {
    let names = case["domains"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    let mut actual = rows.iter().map(|row| {
        (row.values.clone(), row.source_row_ids.clone())
    }).collect::<Vec<_>>();
    let mut expected = case["expected_terminal"].as_array().unwrap().iter().map(|row| {
        let values = names.iter().map(|name| (name.clone(), literal(&row[name]))).collect::<BTreeMap<_,_>>();
        let ids = row["source_ids"].as_array().unwrap().iter()
            .map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>();
        (values, ids)
    }).collect::<Vec<_>>();
    actual.sort_by(|a,b| a.1.cmp(&b.1));
    expected.sort_by(|a,b| a.1.cmp(&b.1));
    assert_eq!(actual, expected, "{id}/{dialect}: complete source-backed output and occurrence identities");
}

#[test]
fn every_shared_sql_form_has_same_complete_canonical_outcome_in_all_exposed_dialects() {
    let freeze: Value = serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap();
    let mut exact_cells = 0;
    let mut residual_cells = 0;
    for case in freeze["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let scripts = case["sql_statements"].as_array()
            .map(|items| items.iter().map(|item| item.as_str().unwrap()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![case["sql"].as_str().unwrap()]);
        for name in DIALECTS {
            let dialect = dialect_from_name(name).unwrap();
            if scripts.iter().any(|sql| Parser::parse_sql(dialect.as_ref(),sql).is_err()) {
                assert!(case["parser_cells"].as_array().unwrap().iter().any(|cell|
                    cell["dialect"] == *name && cell["parser"] == "parser_boundary"));
                continue;
            }
            let sqls = scripts.iter().map(|sql| SqlInput::inline(*sql)).collect::<Vec<_>>();
            let ids = (0..sqls.len()).map(|i| format!("script-{i}")).collect::<Vec<_>>();
            let configured = sqls.iter().zip(&ids).map(|(sql, id)|
                ConfiguredSqlInput::new(id, sql, name, dialect.as_ref())
            ).collect::<Vec<_>>();
            let (catalog, states) = source_evidence(case);
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
            let layer = bundle.layers().last().unwrap();
            let outcome = if id.contains("MULTISTEP") {
                match physical_ordered_cte_read_plan(&bundle, layer.id(), &states, None) {
                    PhysicalOrderedCteProof::Exact(proof) => Some(proof.read.terminal),
                    other => panic!("{id}/{name}: source-derived materialization lost exactness {other:?}"),
                }
            } else if id == "78-C144-P-VIEW" {
                match physical_finite_view_read_plan(&bundle, layer.id(), &states, None) {
                    PhysicalViewProof::Exact(proof) => Some(proof.terminal),
                    other => panic!("{id}/{name}: logical view lost exactness {other:?}"),
                }
            } else {
                match physical_finite_cte_bag_plan(&bundle, layer.id(), &states, None) {
                    PhysicalCteProof::Exact(proof) => Some(proof.terminal),
                    PhysicalCteProof::Residual { .. } if case["classification"] == "conditional_residual" => None,
                    other => panic!("{id}/{name}: canonical physical CTE proof differs {other:?}"),
                }
            };
            match outcome {
                Some(rows) => {
                    assert_eq!(case["classification"], "mandatory_exact", "{id}/{name}");
                    assert_terminal(id, name, &rows, case);
                    exact_cells += 1;
                }
                None => {
                    assert_eq!(case["classification"], "conditional_residual", "{id}/{name}");
                    residual_cells += 1;
                }
            }
        }
    }
    assert_eq!(exact_cells, 7 * DIALECTS.len(), "all seven mandatory physical case families");
    assert_eq!(residual_cells, 2 * DIALECTS.len(), "two reviewed unsupported conditional families");
}
