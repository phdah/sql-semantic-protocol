//! TASK-143: parser boundaries and canonical shared CTE semantics over all exposed dialects.

mod common;
use common::DIALECTS;
use std::collections::BTreeMap;

use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::parser::Parser;
use serde_json::Value;

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
        let id=family["id"].as_str().unwrap();
        let sql=family["sql"].as_str().unwrap();
        let cells=family["cells"].as_array().unwrap();
        assert_eq!(cells.len(),DIALECTS.len(),"{id}: parser cell coverage");
        for name in DIALECTS {
            let matches=cells.iter().filter(|cell|cell["dialect"] == *name).collect::<Vec<_>>();
            let [cell]=matches.as_slice() else {
                panic!("{id}/{name}: missing or duplicate exact dialect cell");
            };
            let dialect=dialect_from_name(name).unwrap();
            let parser=if Parser::parse_sql(dialect.as_ref(),sql).is_ok(){
                "parsed"
            } else {
                "parser_rejected"
            };
            if cell["parser"] != parser {
                mismatches.push(format!("{id}/{name}={parser}"));
            }
            observed.push((id.to_string(),name.to_string()));
        }
    }
    assert_eq!(observed.len(),families.len()*DIALECTS.len());
    assert!(mismatches.is_empty(),
        "TASK-143 frozen parser matrix mismatches (actual statuses): {}",
        mismatches.join(","));
}
