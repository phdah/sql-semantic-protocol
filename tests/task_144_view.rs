//! TASK-144: a logical VIEW is evaluated from the original complete leaf,
//! never accepted as a separate materialized source or writable target.

use std::collections::BTreeMap;
use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_view_read_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalViewProof, PhysicalWriteRow, PhysicalWriteState,
    RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn number(value: i64) -> LiteralValue {
    LiteralValue::Number(value.to_string())
}

fn analyze() -> sql_semantic_protocol::AnalysisBundle {
    let schema = RelationSchema::new("raw".to_string(), vec![
        SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()
    ]).unwrap();
    let catalog = RelationCatalog::from_schemas(&[schema]).unwrap();
    let freeze: Value = serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap();
    let case = freeze["cases"].as_array().unwrap().iter()
        .find(|case| case["id"] == "78-C144-P-VIEW").unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let sqls = case["sql_statements"].as_array().unwrap().iter()
        .map(|item| SqlInput::inline(item.as_str().unwrap())).collect::<Vec<_>>();
    let configured = [
        ConfiguredSqlInput::new("view", &sqls[0], "duckdb", dialect.as_ref()),
        ConfiguredSqlInput::new("read", &sqls[1], "duckdb", dialect.as_ref()),
    ];
    analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap()
}

fn initial(raw: &[Option<i64>]) -> Vec<PhysicalWriteState> {
    vec![PhysicalWriteState::new("raw", raw.iter().map(|value| {
        PhysicalWriteRow::new(BTreeMap::from([
            ("k".to_string(), value.map_or(LiteralValue::Null, number))
        ])).unwrap()
    }).collect()).unwrap()]
}

fn native(raw: &[Option<i64>]) -> Vec<i64> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE raw(k INTEGER)").unwrap();
    for value in raw {
        db.execute("INSERT INTO raw(k) VALUES (?1)", params![value]).unwrap();
    }
    db.execute_batch("CREATE VIEW view_stage AS SELECT k, k + 10 AS shifted FROM raw WHERE k > 1").unwrap();
    db.prepare("SELECT shifted FROM view_stage ORDER BY shifted")
        .unwrap().query_map([], |row| row.get(0)).unwrap()
        .collect::<duckdb::Result<Vec<i64>>>().unwrap()
}

#[test]
fn logical_view_exactly_inherits_source_ids_and_domain_bounds() {
    let bundle = analyze();
    let last = bundle.layers().last().unwrap();
    let snapshots = initial(&[Some(1), Some(2), Some(3)]);
    let result = physical_finite_view_read_plan(&bundle, last.id(), &snapshots, None);
    let PhysicalViewProof::Exact(proof) = result else {
        panic!("TASK-144 VIEW exact bag missing: {result:?}");
    };
    assert_eq!(proof.view_name, "view_stage");
    assert_eq!(proof.source_bag.len(), 3);
    assert_eq!(proof.view_bag.len(), 2);
    assert_eq!(proof.terminal.len(), 2);
    assert_eq!(proof.view_bag[0].source_row_ids, vec!["raw#1"]);
    assert_eq!(proof.view_bag[1].source_row_ids, vec!["raw#2"]);
    let values = proof.terminal.iter().map(|row| {
        let LiteralValue::Number(value) = &row.values["shifted"] else {
            panic!("VIEW output must be integer")
        };
        value.parse::<i64>().unwrap()
    }).collect::<Vec<_>>();
    assert_eq!(values, native(&[Some(1), Some(2), Some(3)]));
    assert_eq!(values, [12, 13]);
    assert_eq!(proof.terminal_domains["shifted"].integer_bounds, Some((12, 13)));
    assert!(!proof.terminal_domains["shifted"].null_allowed);
    let absent = BTreeMap::from([("shifted".to_string(), number(11))]);
    assert!(matches!(
        physical_finite_view_read_plan(&bundle, last.id(), &snapshots, Some(&absent)),
        PhysicalViewProof::Impossible { .. }
    ));
}

#[test]
fn logical_view_re_evaluates_changed_leaf_and_excludes_false_and_unknown() {
    let bundle = analyze();
    let last = bundle.layers().last().unwrap();
    let snapshots = initial(&[Some(1), Some(2), Some(2), None]);
    let result = physical_finite_view_read_plan(&bundle, last.id(), &snapshots, None);
    let PhysicalViewProof::Exact(proof) = result else {
        panic!("VIEW must evaluate duplicate physical rows from the same leaf: {result:?}");
    };
    assert_eq!(proof.source_bag.len(), 4);
    assert_eq!(proof.view_bag.len(), 2);
    assert_eq!(proof.terminal.len(), 2);
    assert_eq!(proof.terminal[0].source_row_ids, ["raw#1"]);
    assert_eq!(proof.terminal[1].source_row_ids, ["raw#2"]);
    assert_eq!(native(&[Some(1), Some(2), Some(2), None]), [12, 12]);
    assert_eq!(proof.terminal_domains["shifted"].integer_bounds, Some((12, 12)));
}

#[test]
fn view_cannot_be_supplied_as_an_independent_materialized_initial_state() {
    let bundle = analyze();
    let last = bundle.layers().last().unwrap();
    let mut snapshots = initial(&[Some(2)]);
    snapshots.push(PhysicalWriteState::new("view_stage", vec![]).unwrap());
    assert!(matches!(
        physical_finite_view_read_plan(&bundle, last.id(), &snapshots, None),
        PhysicalViewProof::Residual { .. }
    ));
}
