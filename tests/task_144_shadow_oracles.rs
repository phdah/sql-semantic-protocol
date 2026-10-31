//! TASK-144 independently re-certifies TASK-78's shadowing oracle using
//! one complete pair of physical input bags and a native DuckDB execution.

use std::collections::{BTreeMap, BTreeSet};

use duckdb::{params, Connection};
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_configured_inputs_with_catalog, dialect_from_name, physical_finite_cte_bag_plan,
    ConfiguredSqlInput, LiteralValue, PhysicalCteProof, PhysicalCteRef, PhysicalWriteRow,
    PhysicalWriteState, RelationCatalog, RelationSchema, SchemaColumn, SqlInput,
};

fn number(v: i64) -> LiteralValue {
    LiteralValue::Number(v.to_string())
}

#[test]
fn shadowed_nested_cte_uses_only_inner_physical_source_and_full_duckdb_bags() {
    let freeze: Value = serde_json::from_str(include_str!("../docs/task-144-acceptance-freeze.json")).unwrap();
    let case = freeze["cases"].as_array().unwrap().iter()
        .find(|case| case["id"] == "78-C144-P-SHADOW").unwrap();
    let mut schemas = Vec::new();
    let mut physical = Vec::new();
    let db = Connection::open_in_memory().unwrap();
    for (name, bag) in case["initial_bags"].as_object().unwrap() {
        schemas.push(RelationSchema::new(name.clone(), vec![
            SchemaColumn::from_sql_type("k", "INTEGER", "duckdb").unwrap()
        ]).unwrap());
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        db.execute_batch(&format!("CREATE TABLE {name}(k INTEGER)")).unwrap();
        let mut rows = Vec::new();
        for value in bag.as_array().unwrap() {
            let k = value.as_i64().unwrap();
            db.execute(&format!("INSERT INTO {name}(k) VALUES (?1)"), params![k]).unwrap();
            rows.push(PhysicalWriteRow::new(BTreeMap::from([("k".to_string(), number(k))])).unwrap());
        }
        physical.push(PhysicalWriteState::new(name.clone(), rows).unwrap());
    }
    let catalog = RelationCatalog::from_schemas(&schemas).unwrap();
    let dialect = dialect_from_name("duckdb").unwrap();
    let sql = SqlInput::inline(case["sql"].as_str().unwrap());
    let configured = [ConfiguredSqlInput::new("shadow", &sql, "duckdb", dialect.as_ref())];
    let bundle = analyze_configured_inputs_with_catalog(&configured, &catalog).unwrap();
    let layer = bundle.layers().last().unwrap();
    let result = physical_finite_cte_bag_plan(&bundle, layer.id(), &physical, None);
    let PhysicalCteProof::Exact(proof) = result else {
        panic!("shadowed complete CTE proof must be Exact: {result:?}");
    };
    assert_eq!(proof.sources, ["inner_src", "outer_src"]);
    assert_eq!(proof.source_bags["outer_src"].len(), 1);
    assert_eq!(proof.source_bags["inner_src"].len(), 2);
    assert_eq!(proof.terminal.len(), 2);
    let mut nodes = BTreeSet::new();
    for node in &proof.nodes {
        assert!(nodes.insert(node.id.clone()), "source and lexical producer IDs must be unique");
        for input in &node.inputs {
            assert!(nodes.contains(&input.reference), "each input must precede its consumer");
        }
    }
    assert!(nodes.contains(&PhysicalCteRef::Source("outer_src".to_string())));
    assert!(nodes.contains(&PhysicalCteRef::Source("inner_src".to_string())));
    let expected = case["expected_terminal"].as_array().unwrap().iter().map(|row| (
        number(row["k"].as_i64().unwrap()),
        row["source_ids"].as_array().unwrap().iter().map(|id| id.as_str().unwrap().to_string()).collect::<Vec<_>>(),
    )).collect::<Vec<_>>();
    let actual = proof.terminal.iter().map(|row| (
        row.values["k"].clone(), row.source_row_ids.clone()
    )).collect::<Vec<_>>();
    assert_eq!(actual, expected, "the outer CTE is shadowed rather than reused");
    assert_eq!(proof.terminal_domains["k"].integer_bounds, Some((2,3)));
    assert!(!proof.terminal_domains["k"].null_allowed);
    let mut query = db.prepare(case["sql"].as_str().unwrap()).unwrap();
    let output = query.query_map([], |row| row.get(0)).unwrap()
        .collect::<duckdb::Result<Vec<i64>>>().unwrap();
    assert_eq!(output, [2,3], "native complete shadowed CTE bag");
    let absent = BTreeMap::from([("k".to_string(), number(10))]);
    assert!(matches!(
        physical_finite_cte_bag_plan(&bundle, layer.id(), &physical, Some(&absent)),
        PhysicalCteProof::Impossible { .. }
    ));
}
