//! TASK-161: independent DuckDB source, intermediate, and terminal oracles.
//! Every full physical catalog is checked at its actual statement checkpoint,
//! not only the final cardinality or a subset of the generated rows.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    analyze_inputs, dialect_from_name, physical_sequential_state_graph_plan, LiteralValue,
    PhysicalSequentialStateGraphProof, PhysicalWriteRow, PhysicalWriteState, SqlInput,
};

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-161-acceptance-freeze.json")).unwrap()
}

fn analyze(sql: &[&str]) -> sql_semantic_protocol::AnalysisBundle {
    let dialect = dialect_from_name("duckdb").unwrap();
    let inputs = sql
        .iter()
        .map(|sql| SqlInput::inline(*sql))
        .collect::<Vec<_>>();
    analyze_inputs(&inputs, "duckdb", dialect.as_ref()).unwrap()
}

fn typed_row(value: &Value) -> PhysicalWriteRow {
    PhysicalWriteRow::new(
        value
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    value
                        .as_i64()
                        .map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                )
            })
            .collect(),
    )
    .unwrap()
}

fn native_rows(db: &Connection, sql: &str, columns: &[String]) -> Vec<PhysicalWriteRow> {
    let mut query = db.prepare(sql).unwrap();
    query
        .query_map([], |row| {
            let values = columns
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let value: Option<i64> = row.get(i)?;
                    Ok((
                        name.to_string(),
                        value.map_or(LiteralValue::Null, |n| LiteralValue::Number(n.to_string())),
                    ))
                })
                .collect::<duckdb::Result<BTreeMap<_, _>>>()?;
            Ok(PhysicalWriteRow::new(values).unwrap())
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap()
}

fn full_bag(rows: &[PhysicalWriteRow]) -> Vec<String> {
    let mut rows = rows
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn physical_names(db: &Connection, relation: &str) -> Vec<String> {
    let mut statement = db.prepare(
        "SELECT column_name FROM information_schema.columns WHERE table_name=?1 ORDER BY ordinal_position"
    ).unwrap();
    statement
        .query_map([relation], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap()
}

fn assert_native_catalog(
    db: &Connection,
    states: &[PhysicalWriteState],
    schemas: &[sql_semantic_protocol::RelationSchema],
) {
    let mut names = states
        .iter()
        .map(|s| s.relation().to_string())
        .collect::<Vec<_>>();
    names.sort();
    let mut native = Vec::new();
    for relation in ["s", "t"] {
        if !physical_names(db, relation).is_empty() {
            native.push(relation.to_string());
        }
    }
    assert_eq!(names, native, "complete live catalog names differ");
    assert_eq!(states.len(), schemas.len());
    for state in states {
        let columns = physical_names(db, state.relation());
        let schema = schemas
            .iter()
            .find(|s| s.relation() == state.relation())
            .unwrap();
        assert_eq!(
            columns,
            schema
                .columns()
                .iter()
                .map(|c| c.name().to_string())
                .collect::<Vec<_>>(),
            "{} physical schema columns",
            state.relation()
        );
        let query = format!("SELECT * FROM {} ORDER BY rowid", state.relation());
        assert_eq!(
            full_bag(state.rows()),
            full_bag(&native_rows(db, &query, &columns)),
            "{} complete rows",
            state.relation()
        );
        let domains = state.exact_domains(schema).unwrap();
        for domain in domains {
            let native = native_rows(db, &query, &columns);
            let mut frequencies = BTreeMap::<String, u64>::new();
            for row in native {
                let value = row.values().get(domain.column()).unwrap();
                *frequencies.entry(format!("{value:?}")).or_default() += 1;
            }
            let attested = domain
                .values()
                .iter()
                .map(|(value, count)| (format!("{value:?}"), *count))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(
                attested,
                frequencies,
                "{}.{} domain",
                state.relation(),
                domain.column()
            );
        }
    }
}

fn check_bounds(domain: &sql_semantic_protocol::PhysicalWriteColumnDomain, expected: &Value) {
    let mut values = Vec::new();
    let mut nulls = 0_u64;
    for (value, frequency) in domain.values() {
        match value {
            LiteralValue::Null => nulls += frequency,
            LiteralValue::Number(number) => values.push(number.parse::<i64>().unwrap()),
            other => panic!("unproved native integer/NULL value {other:?}"),
        }
    }
    assert_eq!(nulls, expected["nulls"].as_u64().unwrap());
    assert_eq!(values.iter().min().copied(), expected["lower"].as_i64());
    assert_eq!(values.iter().max().copied(), expected["upper"].as_i64());
    assert_eq!(expected["lower_inclusive"], true);
    assert_eq!(expected["upper_inclusive"], true);
}

#[test]
fn all_frozen_exact_families_have_independent_full_catalog_duckdb_oracles() {
    let freeze = frozen();
    for case in freeze["cases"].as_array().unwrap().iter().take(3) {
        let program = case["program"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>();
        let proof = physical_sequential_state_graph_plan(&analyze(&program), &[]);
        let PhysicalSequentialStateGraphProof::Exact(exact) = proof else {
            panic!("{} must certify an exact graph: {proof:?}", case["id"]);
        };
        assert_eq!(exact.nodes.len(), program.len());
        let db = Connection::open_in_memory().unwrap();
        for (index, sql) in program.iter().enumerate() {
            let node = &exact.nodes[index];
            assert_native_catalog(&db, &node.before.states, &node.before.schemas);
            if node.read.is_none() {
                db.execute_batch(sql).unwrap();
            }
            assert_native_catalog(&db, &node.after.states, &node.after.schemas);
            if let Some(read) = &node.read {
                let columns = read
                    .output_schema
                    .columns()
                    .iter()
                    .map(|c| c.name().to_string())
                    .collect::<Vec<_>>();
                assert_eq!(
                    full_bag(read.output.rows()),
                    full_bag(&native_rows(&db, sql, &columns)),
                    "{} read step {index}",
                    case["id"]
                );
                assert_eq!(
                    read.surviving_source_ordinals.len() + read.rejected_source_ordinals.len(),
                    read.source.rows().len(),
                    "{} TRUE/NOT TRUE partition",
                    case["id"]
                );
            }
        }
        assert_native_catalog(
            &db,
            &exact.final_states,
            &exact.nodes.last().unwrap().after.schemas,
        );
        match case["id"].as_str().unwrap() {
            "84-C161-P-dml" => {
                let expected = &case["expected"];
                assert_eq!(
                    exact.final_states[0].rows(),
                    &[typed_row(&expected["states"][5][0])]
                );
                for (step, at) in [(1, "inserted"), (2, "updated"), (4, "deleted")] {
                    let write = exact.nodes[step].write.as_ref().unwrap();
                    let count = match at {
                        "inserted" => write.affected().inserted(),
                        "updated" => write.affected().updated(),
                        "deleted" => write.affected().deleted(),
                        _ => unreachable!(),
                    };
                    let expected_count_index = match step {
                        1 => 0,
                        2 => 1,
                        _ => 2,
                    };
                    assert_eq!(
                        count,
                        expected["counts"][expected_count_index][at]
                            .as_u64()
                            .unwrap()
                    );
                }
                let read = exact.nodes[3].read.as_ref().unwrap();
                assert_eq!(read.surviving_source_ordinals, vec![0]);
                assert_eq!(read.rejected_source_ordinals, vec![1, 2]);
                let domains = exact.final_states[0]
                    .exact_domains(&exact.nodes.last().unwrap().after.schemas[0])
                    .unwrap();
                for domain in &domains {
                    check_bounds(domain, &expected["terminal_domains"][domain.column()]);
                }
            }
            "84-C161-P-merge" => {
                let target = exact
                    .final_states
                    .iter()
                    .find(|s| s.relation() == "t")
                    .unwrap();
                assert_eq!(
                    full_bag(target.rows()),
                    full_bag(
                        &case["expected"]["target_rows"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(typed_row)
                            .collect::<Vec<_>>()
                    )
                );
                let domains = target
                    .exact_domains(
                        exact
                            .nodes
                            .last()
                            .unwrap()
                            .after
                            .schemas
                            .iter()
                            .find(|s| s.relation() == "t")
                            .unwrap(),
                    )
                    .unwrap();
                for domain in &domains {
                    check_bounds(
                        domain,
                        &case["expected"]["terminal_domains"][domain.column()],
                    );
                }
            }
            "84-C161-P-ddl" => {
                assert_eq!(exact.final_states.len(), 0);
                assert_eq!(
                    exact.nodes[3]
                        .read
                        .as_ref()
                        .unwrap()
                        .surviving_source_ordinals,
                    vec![0, 1, 2]
                );
                assert_eq!(
                    exact.nodes[3]
                        .read
                        .as_ref()
                        .unwrap()
                        .rejected_source_ordinals,
                    Vec::<usize>::new()
                );
                let domains = exact.nodes[3]
                    .read
                    .as_ref()
                    .unwrap()
                    .exact_domains()
                    .unwrap();
                assert_eq!(
                    domains[1].values(),
                    &[(LiteralValue::Number("2".into()), 3)]
                );
            }
            other => panic!("unexpected mandatory exact case: {other}"),
        }
    }
}

#[test]
fn frozen_impossible_programs_preserve_native_last_valid_state() {
    let freeze = frozen();
    for case in freeze["cases"].as_array().unwrap().iter().skip(3).take(2) {
        let program = case["program"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(
            matches!(
                physical_sequential_state_graph_plan(&analyze(&program), &[]),
                PhysicalSequentialStateGraphProof::Impossible { .. }
            ),
            "{} should be Impossible",
            case["id"]
        );
        let db = Connection::open_in_memory().unwrap();
        for sql in &program[..program.len() - 1] {
            db.execute_batch(sql).unwrap();
        }
        let before = physical_names(&db, "t");
        let native = db.execute_batch(program[program.len() - 1]);
        assert!(
            native.is_err(),
            "{} native contradiction must be real",
            case["id"]
        );
        assert_eq!(physical_names(&db, "t"), before);
        match case["id"].as_str().unwrap() {
            "84-C161-N-duplicate-column" => assert_eq!(before, vec!["a"]),
            "84-C161-N-dropped-read" => assert!(before.is_empty()),
            other => panic!("unexpected impossible family {other}"),
        }
    }
}

#[test]
fn missing_or_session_bound_sources_are_not_synthetic_exact_graphs() {
    let freeze = frozen();
    let missing = freeze["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == "84-C161-N-unknown-producer")
        .unwrap();
    let missing_sql = missing["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|sql| sql.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(matches!(
        physical_sequential_state_graph_plan(&analyze(&missing_sql), &[]),
        PhysicalSequentialStateGraphProof::Residual { .. }
    ));
    let conditional = freeze["cases"].as_array().unwrap().iter()
        .find(|case| case["id"] == "84-C161-C-transaction").unwrap();
    let sql = conditional["program"].as_array().unwrap().iter()
        .map(|sql| sql.as_str().unwrap()).collect::<Vec<_>>();
    let engine = dialect_from_name("duckdb").unwrap();
    let inputs = sql.iter().map(|statement| SqlInput::inline(*statement)).collect::<Vec<_>>();
    if let Ok(bundle) = analyze_inputs(&inputs, "duckdb", engine.as_ref()) {
        assert!(!matches!(
            physical_sequential_state_graph_plan(&bundle, &[]),
            PhysicalSequentialStateGraphProof::Exact(_)
        ), "unverified transaction law must never be promoted to Exact");
    }
}
