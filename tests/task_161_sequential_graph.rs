//! TASK-161: public typed sequential graph, independently checked against
//! full native DuckDB bags and the frozen positive/negative state contract.

use std::collections::BTreeMap;

use duckdb::Connection;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_sequential_state_graph_plan, LiteralValue,
    PhysicalScriptProducer, PhysicalScriptSourceRole, PhysicalSequentialStateGraphProof,
    PhysicalWriteRow, PhysicalWriteState, SqlInput,
};

fn analyze(sql: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let parser = dialect_from_name("duckdb").unwrap();
    let inputs = sql
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    analyze_inputs(&inputs, "duckdb", parser.as_ref()).unwrap()
}

fn row(id: i64, value: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        ("id".to_string(), LiteralValue::Number(id.to_string())),
        (
            "v".to_string(),
            value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
        ),
    ]))
    .unwrap()
}

fn rows(state: &PhysicalWriteState) -> Vec<String> {
    let mut rows = state
        .rows()
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn native_rows(db: &Connection, relation: &str) -> Vec<String> {
    assert!(["t", "s"].contains(&relation));
    let mut statement = db
        .prepare(&format!("SELECT id, v FROM {relation} ORDER BY rowid"))
        .unwrap();
    let result = statement
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap();
    let mut serialized = result
        .iter()
        .map(|(id, value)| {
            let values = BTreeMap::from([
                (
                    "id".to_string(),
                    id.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                ),
                (
                    "v".to_string(),
                    value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                ),
            ]);
            format!("{values:?}")
        })
        .collect::<Vec<_>>();
    serialized.sort();
    serialized
}

#[test]
fn dml_script_links_each_closed_source_version_to_its_statement_and_rejected_rows() {
    let sql = [
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "INSERT INTO t (id, v) VALUES (1, 10), (2, NULL), (2, NULL)",
        "UPDATE t SET v = 20 WHERE id = 1",
        "SELECT id, v FROM t WHERE v >= 20",
        "DELETE FROM t WHERE id = 2",
        "SELECT id, v FROM t",
    ];
    let bundle = analyze(&sql);
    let proof = physical_sequential_state_graph_plan(&bundle, &[]);
    let PhysicalSequentialStateGraphProof::Exact(exact) = proof else {
        panic!("expected complete graph, got {proof:?}");
    };
    assert_eq!(exact.nodes.len(), sql.len());
    assert_eq!(exact.nodes[0].before.states.len(), 0);
    assert_eq!(exact.nodes[1].after.states[0].rows().len(), 3);
    assert_eq!(exact.nodes[2].before, exact.nodes[1].after);
    assert_eq!(exact.nodes[4].before, exact.nodes[3].after);
    assert_eq!(
        exact.nodes[1].write.as_ref().unwrap().affected().inserted(),
        3
    );
    assert_eq!(
        exact.nodes[2].write.as_ref().unwrap().affected().updated(),
        1
    );
    assert_eq!(
        exact.nodes[4].write.as_ref().unwrap().affected().deleted(),
        2
    );

    let first_read = exact.nodes[3].read.as_ref().unwrap();
    assert_eq!(first_read.surviving_source_ordinals, vec![0]);
    assert_eq!(first_read.rejected_source_ordinals, vec![1, 2]);
    assert_eq!(first_read.output.rows(), &[row(1, Some(20))]);
    let second_read = exact.nodes[5].read.as_ref().unwrap();
    assert_eq!(second_read.output.rows(), &[row(1, Some(20))]);
    assert_eq!(second_read.rejected_source_ordinals, Vec::<usize>::new());
    assert_eq!(exact.final_states[0].rows(), &[row(1, Some(20))]);
    let domains = exact.nodes[5].after.exact_domains().unwrap();
    assert_eq!(domains[0].0, "t");
    assert_eq!(
        domains[0].1[0].values(),
        &[(LiteralValue::Number("1".into()), 1)]
    );
    assert_eq!(
        domains[0].1[1].values(),
        &[(LiteralValue::Number("20".into()), 1)]
    );

    let find = |consumer: &str, role| {
        exact.edges.iter().find(|edge| {
            edge.consumer_layer_id == consumer && edge.relation == "t" && edge.role == role
        })
    };
    let updated = find(
        &exact.nodes[2].layer_id,
        PhysicalScriptSourceRole::TargetPrestate,
    )
    .unwrap();
    assert_eq!(
        updated.producer,
        PhysicalScriptProducer::Statement(exact.nodes[1].layer_id.clone())
    );
    let selected = find(
        &exact.nodes[3].layer_id,
        PhysicalScriptSourceRole::ReadSource,
    )
    .unwrap();
    assert_eq!(
        selected.producer,
        PhysicalScriptProducer::Statement(exact.nodes[2].layer_id.clone())
    );
    let deleted = find(
        &exact.nodes[4].layer_id,
        PhysicalScriptSourceRole::TargetPrestate,
    )
    .unwrap();
    assert_eq!(
        deleted.producer,
        PhysicalScriptProducer::Statement(exact.nodes[2].layer_id.clone())
    );
    let final_read = find(
        &exact.nodes[5].layer_id,
        PhysicalScriptSourceRole::ReadSource,
    )
    .unwrap();
    assert_eq!(
        final_read.producer,
        PhysicalScriptProducer::Statement(exact.nodes[4].layer_id.clone())
    );

    let db = Connection::open_in_memory().unwrap();
    for (index, statement) in sql.iter().enumerate() {
        if index != 3 && index != 5 {
            db.execute_batch(statement).unwrap();
        }
        let actual = native_rows(&db, "t");
        assert_eq!(
            rows(&exact.nodes[index].after.states[0]),
            actual,
            "step {index}"
        );
        if let Some(read) = &exact.nodes[index].read {
            let mut native = db.prepare(statement).unwrap();
            let actual = native
                .query_map([], |row| {
                    Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
                })
                .unwrap()
                .collect::<duckdb::Result<Vec<_>>>()
                .unwrap();
            let expected = actual
                .into_iter()
                .map(|(id, value)| row(id.unwrap(), value))
                .collect::<Vec<_>>();
            assert_eq!(read.output.rows(), expected, "read step {index}");
        }
    }
}

#[test]
fn merge_uses_the_same_source_and_target_states_and_preserves_both_live_relations() {
    let sql = [
        "CREATE TABLE t (id INTEGER, v INTEGER)",
        "INSERT INTO t VALUES (1, 10)",
        "CREATE TABLE s (id INTEGER, v INTEGER)",
        "INSERT INTO s VALUES (1, 20), (2, 30)",
        "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN UPDATE SET v = s.v WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v)",
        "SELECT id, v FROM t",
    ];
    let proof = physical_sequential_state_graph_plan(&analyze(&sql), &[]);
    let PhysicalSequentialStateGraphProof::Exact(exact) = proof else {
        panic!("expected exact MERGE graph, got {proof:?}");
    };
    assert_eq!(exact.nodes.len(), sql.len());
    assert_eq!(exact.nodes[4].before.states.len(), 2);
    assert_eq!(exact.nodes[4].after.states.len(), 2);
    assert_eq!(exact.final_states.len(), 2);
    let target = exact
        .final_states
        .iter()
        .find(|state| state.relation() == "t")
        .unwrap();
    assert_eq!(
        rows(target),
        rows(&PhysicalWriteState::new("t", vec![row(1, Some(20)), row(2, Some(30))]).unwrap())
    );
    assert!(exact.edges.iter().any(|edge| {
        edge.consumer_layer_id == exact.nodes[4].layer_id
            && edge.relation == "s"
            && edge.producer == PhysicalScriptProducer::Statement(exact.nodes[3].layer_id.clone())
    }));
    let db = Connection::open_in_memory().unwrap();
    for sql in &sql[..5] {
        db.execute_batch(sql).unwrap();
    }
    assert_eq!(rows(target), native_rows(&db, "t"));
    assert_eq!(
        rows(
            exact
                .final_states
                .iter()
                .find(|state| state.relation() == "s")
                .unwrap()
        ),
        native_rows(&db, "s")
    );
}

#[test]
fn create_alter_select_drop_has_typed_intermediate_schema_but_no_final_table() {
    let sql = [
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t(a) VALUES (1), (NULL), (1)",
        "ALTER TABLE t ADD COLUMN b INTEGER DEFAULT 2",
        "SELECT a, b FROM t",
        "DROP TABLE t",
    ];
    let proof = physical_sequential_state_graph_plan(&analyze(&sql), &[]);
    let PhysicalSequentialStateGraphProof::Exact(exact) = proof else {
        panic!("expected exact DDL graph, got {proof:?}");
    };
    assert_eq!(exact.nodes[2].after.schemas[0].columns().len(), 2);
    assert_eq!(
        exact.nodes[3]
            .read
            .as_ref()
            .unwrap()
            .surviving_source_ordinals,
        vec![0, 1, 2]
    );
    assert!(exact.nodes[4].after.states.is_empty());
    assert!(exact.nodes[4].after.schemas.is_empty());
    assert!(exact.final_states.is_empty());
    let domains = exact.nodes[3].before.exact_domains().unwrap();
    assert_eq!(
        domains[0].1[0].values(),
        &[
            (LiteralValue::Number("1".into()), 2),
            (LiteralValue::Null, 1)
        ]
    );
    assert_eq!(
        domains[0].1[1].values(),
        &[(LiteralValue::Number("2".into()), 3)]
    );
}

#[test]
fn contradictory_ddl_and_known_dropped_read_are_impossible_but_missing_sources_are_residual() {
    let duplicate = analyze(&[
        "CREATE TABLE t (a INTEGER)",
        "INSERT INTO t(a) VALUES (1)",
        "ALTER TABLE t ADD COLUMN a INTEGER",
    ]);
    assert!(matches!(
        physical_sequential_state_graph_plan(&duplicate, &[]),
        PhysicalSequentialStateGraphProof::Impossible { .. }
    ));
    let dropped = analyze(&[
        "CREATE TABLE t(a INTEGER)",
        "DROP TABLE t",
        "SELECT a FROM t",
    ]);
    assert!(matches!(
        physical_sequential_state_graph_plan(&dropped, &[]),
        PhysicalSequentialStateGraphProof::Impossible { .. }
    ));
    let missing = analyze(&["SELECT a FROM unprovided_t"]);
    assert!(matches!(
        physical_sequential_state_graph_plan(&missing, &[]),
        PhysicalSequentialStateGraphProof::Residual { .. }
    ));
}

#[test]
fn acceptance_scope_is_frozen_before_implementation_and_parent_signoff_remains_open() {
    let freeze: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-161-acceptance-freeze.json")).unwrap();
    assert_eq!(freeze["owner"], "TASK-161");
    assert_eq!(freeze["version"], 2);
    let original: serde_json::Value =
        serde_json::from_str(include_str!("../docs/task-161-acceptance-freeze-v1.json")).unwrap();
    assert_eq!(original["version"], 1);
    assert_eq!(original["frozen_before_semantic_changes"], true);
    assert_eq!(
        freeze["history"][0],
        "docs/task-161-acceptance-freeze-v1.json"
    );
    assert_eq!(freeze["frozen_before_semantic_changes"], true);
    assert_eq!(freeze["parser_dialect_cells"], 91);
    let ids = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(ids.contains("84-C161-P-dml"));
    assert!(ids.contains("84-C161-P-merge"));
    assert!(ids.contains("84-C161-P-ddl"));
    assert!(ids.contains("84-C161-N-dropped-read"));
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    assert_eq!(
        manifest["task_161_sequential_state_graph_acceptance"]["freeze"],
        "docs/task-161-acceptance-freeze.json"
    );
    assert_eq!(
        manifest["task_161_sequential_state_graph_acceptance"]["parent_signed_off"],
        false
    );
}
