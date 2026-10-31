//! TASK-144: every named frozen SQL shape across every public dialect identity.
//! Parser acceptance is not a native engine assertion. Printed cell statuses
//! are used to freeze the reviewed executable matrix, never silently skipped.

mod common;

use common::DIALECTS;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};
use sqlparser::parser::Parser;
use std::collections::{BTreeMap, BTreeSet};

fn source() -> (RelationCatalog, Vec<PhysicalWriteState>) {
    let catalog = RelationCatalog::from_schemas(&[
        RelationSchema::new(
            "raw".to_string(),
            vec![SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()],
        )
        .unwrap(),
        RelationSchema::new(
            "target".to_string(),
            vec![
                SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap(),
                SchemaColumn::from_sql_type("shifted", "INTEGER", "duckdb").unwrap(),
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let states = vec![
        PhysicalWriteState::new(
            "raw".to_string(),
            [1, 2, 3]
                .iter()
                .map(|k| {
                    PhysicalWriteRow::new(BTreeMap::from([(
                        "k".to_string(),
                        LiteralValue::Number(k.to_string()),
                    )]))
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        PhysicalWriteState::new("target".to_string(), vec![]).unwrap(),
    ];
    (catalog, states)
}

#[test]
fn freeze_names_all_public_dialects_once() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap();
    let names = freeze["dialects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(names.len(), DIALECTS.len());
    assert_eq!(names, DIALECTS.iter().copied().collect::<BTreeSet<_>>());
    let cases = freeze["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 9);
    let ids = cases
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), cases.len(), "duplicate frozen fixture identity");
    for case in cases {
        let required = case["required_dialect_names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d.as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(required, names, "{}: missing dialect", case["id"]);
        assert_eq!(case["adapter_cells"].as_array().unwrap().len(), 3);
    }
}

#[test]
fn every_frozen_variant_has_an_explicit_runtime_parser_boundary() {
    let freeze: Value =
        serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap();
    let cases = freeze["cases"].as_array().unwrap();
    let mut cells = 0;
    let mut statuses = BTreeMap::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let sql = case["sql_statements"]
            .as_array()
            .map(|lines| {
                lines
                    .iter()
                    .map(|line| line.as_str().unwrap())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![case["sql"].as_str().unwrap()]);
        for name in DIALECTS {
            let dialect = dialect_from_name(name).unwrap();
            let parsed = sql
                .iter()
                .all(|statement| Parser::parse_sql(dialect.as_ref(), statement).is_ok());
            let status = if parsed { "parsed" } else { "parser_boundary" };
            let frozen = case["parser_cells"].as_array().unwrap().iter().filter(|cell| cell["dialect"] == *name).collect::<Vec<_>>();
            let [cell] = frozen.as_slice() else {
                panic!("{id}/{name}: duplicate or missing frozen parser classification");
            };
            assert_eq!(cell["parser"], status, "{id}/{name}: reviewed parser classification differs from executable result");
            statuses.insert(format!("{id}/{name}"), status);
            // All shared portable read cases must carry the same semantics
            // when the dialect has accepted their AST shape.
            if parsed
                && matches!(
                    id,
                    "78-C144-P-REUSED-FILTER" | "78-C144-N-NULL-DUP" | "78-C144-P-FINITE-RECURSION"
                )
            {
                let (catalog, states) = source();
                let sql = SqlInput::inline(sql[0]);
                let configured = [ConfiguredSqlInput::new(
                    "dialect",
                    &sql,
                    name,
                    dialect.as_ref(),
                )];
                let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
                let layer = bundle.layers().last().unwrap();
                let leaves = if id.contains("RECURSION") {
                    &[][..]
                } else {
                    &states[..1]
                };
                assert!(
                    matches!(
                        physical_finite_cte_bag_plan(&bundle, layer.id(), leaves, None),
                        PhysicalCteProof::Exact(_)
                    ),
                    "{id}/{name}: accepted shared CTE syntax must retain canonical proof"
                );
            }
            cells += 1;
        }
    }
    assert_eq!(cells, cases.len() * DIALECTS.len());
    for (key, status) in &statuses {
        println!("TASK-144 PARSER CELL {key}={status}");
    }
}
