//! TASK-161: the exact task-owned four-snapshot numerical oracle.
//! Initial empty CREATE, INSERT [1,2], UPDATE key1 and DELETE key2 must
//! construct one updated terminal row without independently supplied stages.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_sequential_state_graph_plan,
    LiteralValue, PhysicalSequentialStateGraphProof, PhysicalWriteRow, SqlInput,
};

fn row(id: i64, amount: i64) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        ("amount".to_string(), LiteralValue::Number(amount.to_string())),
    ]))
    .unwrap()
}

#[test]
fn four_sequential_source_complete_snapshots_match_native_duckdb() {
    let sql = [
        "CREATE TABLE t(id INTEGER, amount INTEGER)",
        "INSERT INTO t VALUES (1, 10), (2, 20)",
        "UPDATE t SET amount = 15 WHERE id = 1",
        "DELETE FROM t WHERE id = 2",
    ];
    let dialect = dialect_from_name("duckdb").unwrap();
    let input = sql
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    let bundle = analyze_inputs(&input, "duckdb", dialect.as_ref()).unwrap();
    let proof = physical_sequential_state_graph_plan(&bundle, &[]);
    let PhysicalSequentialStateGraphProof::Exact(graph) = proof else {
        panic!("the frozen finite source-state oracle must be exact: {proof:?}");
    };
    let expected = [
        vec![],
        vec![row(1, 10), row(2, 20)],
        vec![row(1, 15), row(2, 20)],
        vec![row(1, 15)],
    ];
    assert_eq!(graph.nodes.len(), 4);
    let db = Connection::open_in_memory().unwrap();
    for (index, statement) in sql.iter().enumerate() {
        db.execute_batch(statement).unwrap();
        let snapshot = &graph.nodes[index];
        assert_eq!(snapshot.after.states[0].rows(), expected[index], "snapshot {index}");
        assert_eq!(
            snapshot.after.exact_domains().unwrap()[0].1.len(),
            2,
            "schema-backed id and amount domains"
        );
        let mut native = db.prepare("SELECT id, amount FROM t ORDER BY rowid").unwrap();
        let observed = native.query_map([], |r| {
            Ok(row(r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        }).unwrap().collect::<duckdb::Result<Vec<_>>>().unwrap();
        assert_eq!(snapshot.after.states[0].rows(), observed, "native {index}");
        if index > 0 {
            assert_eq!(snapshot.before, graph.nodes[index - 1].after);
        }
    }
    assert_eq!(graph.nodes[1].write.as_ref().unwrap().affected().inserted(), 2);
    assert_eq!(graph.nodes[2].write.as_ref().unwrap().affected().updated(), 1);
    assert_eq!(graph.nodes[2].write.as_ref().unwrap().affected_target_ordinals(), &[0]);
    assert_eq!(graph.nodes[3].write.as_ref().unwrap().affected().deleted(), 1);
    assert_eq!(graph.nodes[3].write.as_ref().unwrap().affected_target_ordinals(), &[1]);
    assert_eq!(graph.final_states[0].rows(), &[row(1, 15)]);
    let domains = graph.final_states[0]
        .exact_domains(&graph.nodes[3].after.schemas[0]).unwrap();
    assert_eq!(domains[0].values(), &[(LiteralValue::Number("1".into()), 1)]);
    assert_eq!(domains[1].values(), &[(LiteralValue::Number("15".into()), 1)]);
}
