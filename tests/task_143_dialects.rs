//! TASK-143: parser boundaries and canonical shared CTE semantics over all exposed dialects.

mod common;
use common::DIALECTS;
use std::collections::BTreeMap;

use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    physical_ordered_write_state_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    PhysicalWriteStatePlan,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::parser::Parser;

const SQL: [&str; 2] = [
    "WITH stage AS (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1) \
     SELECT a.shifted AS l, b.shifted AS r FROM stage a JOIN stage b ON a.k = b.k",
    "WITH stage AS (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1) \
     SELECT COUNT(*) AS total FROM stage a JOIN stage b ON a.k = b.k",
];

#[test]
fn accepted_dialect_syntax_preserves_exact_physical_source_bags() {
    let source_schema = RelationSchema::new(
        "raw".to_string(),
        vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
    )
    .unwrap();
    let catalog = RelationCatalog::from_schemas(&[source_schema]).unwrap();
    let source_rows = [1, 2, 3]
        .into_iter()
        .map(|number| {
            PhysicalWriteRow::new(BTreeMap::from([(
                "k".to_string(),
                LiteralValue::Number(number.to_string()),
            )]))
            .unwrap()
        })
        .collect();
    let snapshots = [PhysicalWriteState::new("raw".to_string(), source_rows).unwrap()];
    let mut accepted = 0;
    let mut rejected = 0;
    for sql in SQL {
        for dialect_name in DIALECTS {
            let dialect = dialect_from_name(dialect_name).unwrap();
            if Parser::parse_sql(dialect.as_ref(), sql).is_err() {
                rejected += 1;
                continue;
            }
            let source = SqlInput::inline(sql);
            let configured = [ConfiguredSqlInput::new(
                "cte",
                &source,
                dialect_name,
                dialect.as_ref(),
            )];
            let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
            let target = bundle.layers().last().unwrap();
            let PhysicalCteProof::Exact(proof) =
                physical_finite_cte_bag_plan(&bundle, target.id(), &snapshots, None)
            else {
                panic!("TASK-143 {dialect_name}: parsed but no canonical proof");
            };
            assert_eq!(proof.source_bags["raw"].len(), 3);
            assert_eq!(proof.stage_bags["terminal/cte:0:stage"].len(), 2);
            assert_eq!(
                proof.terminal.len(),
                if sql.contains("COUNT(") { 1 } else { 2 }
            );
            if sql.contains("COUNT(") {
                assert_eq!(
                    proof.terminal[0].values["total"],
                    LiteralValue::Number("2".to_string())
                );
            }
            accepted += 1;
        }
    }
    assert_eq!(accepted + rejected, SQL.len() * DIALECTS.len());
    assert!(
        accepted > 0,
        "no parser supports the shared portable CTE syntax"
    );
}

#[test]
fn every_frozen_parser_family_matches_its_exact_dialect_boundary() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-143-acceptance-freeze.json")).unwrap();
    let families = freeze["parser_families"].as_array().unwrap();
    assert_eq!(families.len(), 5, "every named SQL shape must be frozen");
    let mut observed = Vec::new();
    let mut mismatches = Vec::new();
    for family in families {
        let id = family["id"].as_str().unwrap();
        let sql = family["sql"].as_str().unwrap();
        let cells = family["cells"].as_array().unwrap();
        assert_eq!(cells.len(), DIALECTS.len(), "{id}: parser cell coverage");
        for name in DIALECTS {
            let matches = cells
                .iter()
                .filter(|cell| cell["dialect"] == *name)
                .collect::<Vec<_>>();
            let [cell] = matches.as_slice() else {
                panic!("{id}/{name}: missing or duplicate exact dialect cell");
            };
            let dialect = dialect_from_name(name).unwrap();
            let parser = if Parser::parse_sql(dialect.as_ref(), sql).is_ok() {
                "parsed"
            } else {
                "parser_rejected"
            };
            if cell["parser"] != parser {
                mismatches.push(format!("{id}/{name}={parser}"));
            }
            observed.push((id.to_string(), name.to_string()));
        }
    }
    assert_eq!(observed.len(), families.len() * DIALECTS.len());
    assert!(
        mismatches.is_empty(),
        "TASK-143 frozen parser matrix mismatches (actual statuses): {}",
        mismatches.join(",")
    );
}

#[test]
fn all_parser_accepted_dialects_preserve_computed_cte_insert_states() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-143-acceptance-freeze.json")).unwrap();
    let write=freeze["cases"].as_array().unwrap().iter()
        .find(|c|c["id"]=="78-C143-P-WRITE").unwrap();
    let sql=write["sql"].as_str().unwrap();
    let catalog=RelationCatalog::from_schemas(&[
        RelationSchema::new("raw".to_string(),
            vec![SchemaColumn::from_sql_type("k","INTEGER","duckdb").unwrap()]).unwrap(),
        RelationSchema::new("target".to_string(),vec![
            SchemaColumn::from_sql_type("k","INTEGER","duckdb").unwrap(),
            SchemaColumn::from_sql_type("shifted","INTEGER","duckdb").unwrap()
        ]).unwrap(),
    ]).unwrap();
    let raw=PhysicalWriteState::new("raw", [1,2,3].into_iter().map(|k|
        PhysicalWriteRow::new(BTreeMap::from([(
            "k".to_string(),LiteralValue::Number(k.to_string())
        )])).unwrap()
    ).collect()).unwrap();
    let target=PhysicalWriteState::new("target",vec![
        PhysicalWriteRow::new(BTreeMap::from([
            ("k".to_string(),LiteralValue::Number("99".to_string())),
            ("shifted".to_string(),LiteralValue::Number("109".to_string()))
        ])).unwrap()
    ]).unwrap();
    let snapshots=[raw,target];
    let mut dialects=0;
    for name in DIALECTS {
        let dialect=dialect_from_name(name).unwrap();
        assert!(Parser::parse_sql(dialect.as_ref(),sql).is_ok(),"{name}: frozen parser cell");
        let source=SqlInput::inline(sql);
        let configured=[ConfiguredSqlInput::new("task143",&source,name,dialect.as_ref())];
        let bundle=analyze_configured_inputs_with_catalog(&configured,&catalog).unwrap();
        let plan=physical_ordered_write_state_plan(&bundle,&snapshots);
        let PhysicalWriteStatePlan::Feasible{transitions,final_states}=plan else {
            panic!("TASK-143 {name}: source-derived INSERT failed closed unexpectedly: {plan:?}");
        };
        assert_eq!(transitions.len(),1,"{name}: one write");
        assert_eq!(transitions[0].intermediate_states().len(),1,"{name}: one scoped producer");
        assert_eq!(transitions[0].intermediate_states()[0].rows().len(),2,"{name}: filter");
        assert_eq!(transitions[0].affected().inserted(),2,"{name}: affected");
        assert_eq!(transitions[0].after().rows().len(),3,"{name}: complete target");
        assert_eq!(final_states.len(),2,"{name}: no synthetic intermediate source");
        dialects+=1;
    }
    assert_eq!(dialects,DIALECTS.len());
}
