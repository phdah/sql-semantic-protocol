//! TASK-164: frozen ordered DDL, DML, MERGE, SELECT and transaction oracles.
//! Physical source and final bags are compared to an independently executed
//! DuckDB program after every SQL unit, including duplicate and NULL rows.

use std::collections::BTreeMap;

use duckdb::Connection;
use serde_json::Value;
use sql_semantic_protocol::{
    physical_duckdb_sql_transaction_plan, physical_duckdb_sql_transaction_plan_with_laws,
    LiteralValue, PhysicalScriptCatalog, PhysicalTransactionScriptExact,
    PhysicalTransactionScriptProof, PhysicalTransactionSessionLaw, PhysicalWriteRow,
};

fn frozen() -> Value {
    serde_json::from_str(include_str!("../docs/task-164-acceptance-freeze.json")).unwrap()
}

fn row_from_json(value: &Value) -> PhysicalWriteRow {
    let columns = value
        .as_object()
        .expect("frozen row object")
        .iter()
        .map(|(name, value)| {
            (
                name.clone(),
                value.as_i64().map_or(LiteralValue::Null, |number| {
                    LiteralValue::Number(number.to_string())
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    PhysicalWriteRow::new(columns).expect("nonempty row keys")
}

fn row(id: Option<i64>, v: Option<i64>) -> PhysicalWriteRow {
    PhysicalWriteRow::new(BTreeMap::from([
        (
            "id".to_string(),
            id.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
        (
            "v".to_string(),
            v.map_or(LiteralValue::Null, |value| {
                LiteralValue::Number(value.to_string())
            }),
        ),
    ]))
    .unwrap()
}

fn sorted(rows: &[PhysicalWriteRow]) -> Vec<String> {
    let mut bag = rows
        .iter()
        .map(|row| format!("{:?}", row.values()))
        .collect::<Vec<_>>();
    bag.sort();
    bag
}

fn expected_rows(values: &Value) -> Vec<PhysicalWriteRow> {
    values
        .as_array()
        .expect("frozen full bag")
        .iter()
        .map(row_from_json)
        .collect()
}

fn native_rows(db: &Connection, table: &str) -> Vec<PhysicalWriteRow> {
    assert!(matches!(table, "t" | "s"));
    let sql = format!("SELECT id, v FROM {table}");
    let mut statement = db.prepare(&sql).unwrap();
    statement
        .query_map([], |result| {
            Ok(row(
                result.get::<_, Option<i64>>(0)?,
                result.get::<_, Option<i64>>(1)?,
            ))
        })
        .unwrap()
        .collect::<duckdb::Result<Vec<_>>>()
        .unwrap()
}

fn assert_native_catalog(db: &Connection, checkpoint: &PhysicalScriptCatalog, index: usize) {
    assert_eq!(checkpoint.states.len(), checkpoint.schemas.len());
    assert!(
        checkpoint.exact_domains().is_some(),
        "typed checkpoint {index}"
    );
    for table in ["t", "s"] {
        let exists: i64 = db
            .query_row(
                "SELECT count(*) FROM information_schema.tables WHERE table_name = ?1",
                [table],
                |result| result.get(0),
            )
            .unwrap();
        let actual = checkpoint
            .states
            .iter()
            .find(|state| state.relation() == table);
        assert_eq!(
            exists == 1,
            actual.is_some(),
            "{table} presence at step {index}"
        );
        if let Some(state) = actual {
            assert_eq!(
                sorted(state.rows()),
                sorted(&native_rows(db, table)),
                "{table} complete bag after SQL step {index}"
            );
        }
    }
}

fn assert_terminal_domains(exact: &PhysicalTransactionScriptExact, expected: &Value) {
    if expected.is_null() {
        return;
    }
    let domains = exact.final_catalog.exact_domains().unwrap();
    let table = domains
        .iter()
        .find(|(relation, _)| relation == "t")
        .expect("final target table");
    for column in &table.1 {
        let wanted = &expected[column.column()];
        if wanted.is_null() {
            continue;
        }
        let mut integers = Vec::new();
        let mut nulls = 0u64;
        for (value, frequency) in column.values() {
            match value {
                LiteralValue::Number(value) => {
                    integers.push(value.parse::<i64>().unwrap());
                    assert!(*frequency > 0);
                }
                LiteralValue::Null => nulls += frequency,
                other => panic!("TASK-164 integer/NULL oracle received {other:?}"),
            }
        }
        if let Some(lower) = wanted["lower"].as_i64() {
            assert_eq!(integers.iter().min().copied(), Some(lower));
            assert_eq!(wanted["lower_inclusive"], true);
        }
        if let Some(upper) = wanted["upper"].as_i64() {
            assert_eq!(integers.iter().max().copied(), Some(upper));
            assert_eq!(wanted["upper_inclusive"], true);
        }
        if let Some(expected_nulls) = wanted["nulls"].as_u64() {
            assert_eq!(nulls, expected_nulls, "column {}", column.column());
        }
        if let Some(frequencies) = wanted["values"].as_array() {
            let actual = column.values();
            for frequency in frequencies {
                let expected_value = if frequency["value"].is_null() {
                    LiteralValue::Null
                } else {
                    LiteralValue::Number(frequency["value"].as_i64().unwrap().to_string())
                };
                assert_eq!(
                    actual
                        .iter()
                        .find(|(value, _)| *value == expected_value)
                        .map(|(_, n)| *n),
                    Some(frequency["count"].as_u64().unwrap())
                );
            }
        }
    }
}

fn assert_read(statement: &sql_semantic_protocol::PhysicalScriptStateNode, expected: &Value) {
    let read = statement.read.as_ref().expect("certified SQL SELECT");
    assert_eq!(
        sorted(read.output.rows()),
        sorted(&expected_rows(&expected["rows"]))
    );
    let rejected = expected["not_true_source_ordinals"]
        .as_array()
        .expect("explicit NOT TRUE occurrence list")
        .iter()
        .map(|ordinal| ordinal.as_u64().unwrap() as usize)
        .collect::<Vec<_>>();
    assert_eq!(read.rejected_source_ordinals, rejected);
    assert_eq!(
        read.surviving_source_ordinals.len(),
        read.output.rows().len()
    );
    assert!(read.exact_domains().is_some());
}

fn run_native_case(case: &Value) {
    let program = case["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step.as_str().unwrap())
        .collect::<Vec<_>>();
    let proof = physical_duckdb_sql_transaction_plan(
        &program,
        &PhysicalScriptCatalog {
            states: Vec::new(),
            schemas: Vec::new(),
        },
    );
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("{} must be exact: {proof:?}", case["id"]);
    };
    assert_eq!(exact.checkpoints.len(), program.len());

    let db = Connection::open_in_memory().unwrap();
    for (index, sql) in program.iter().enumerate() {
        let checkpoint = &exact.checkpoints[index];
        assert_eq!(checkpoint.step_index, index);
        if let Some(previous) = index.checked_sub(1) {
            assert_eq!(
                checkpoint.before, exact.checkpoints[previous].after,
                "same complete version at step {index}"
            );
        }
        if sql.starts_with("SELECT") {
            let statement = checkpoint.statement.as_ref().unwrap();
            let read = statement.read.as_ref().unwrap();
            let native = db
                .prepare(sql)
                .unwrap()
                .query_map([], |result| {
                    Ok(row(
                        result.get::<_, Option<i64>>(0)?,
                        result.get::<_, Option<i64>>(1)?,
                    ))
                })
                .unwrap()
                .collect::<duckdb::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(sorted(read.output.rows()), sorted(&native));
        } else {
            db.execute_batch(sql)
                .unwrap_or_else(|error| panic!("native SQL step {index} failed: {error}"));
        }
        assert_native_catalog(&db, &checkpoint.after, index);
        if let Some(snapshots) = case["checkpoint_t"].as_array() {
            let expected = expected_rows(&snapshots[index]);
            let actual = checkpoint
                .after
                .states
                .iter()
                .find(|state| state.relation() == "t")
                .expect("t exists after CREATE");
            assert_eq!(
                sorted(actual.rows()),
                sorted(&expected),
                "frozen t step {index}"
            );
        }
        if let Some(write) = checkpoint
            .statement
            .as_ref()
            .and_then(|statement| statement.write.as_ref())
        {
            for (name, value) in [
                ("inserted", write.affected().inserted()),
                ("updated", write.affected().updated()),
                ("deleted", write.affected().deleted()),
            ] {
                if let Some(counts) = case["counts"][name].as_array() {
                    assert_eq!(
                        value,
                        counts[index].as_u64().unwrap(),
                        "{name} step {index}"
                    );
                }
            }
        }
    }
    assert_eq!(exact.final_catalog, exact.checkpoints.last().unwrap().after);
    assert_native_catalog(&db, &exact.final_catalog, program.len());
    for (relation, values) in case["expected_final"].as_object().unwrap() {
        let target = exact
            .final_catalog
            .states
            .iter()
            .find(|state| state.relation() == relation)
            .unwrap();
        assert_eq!(sorted(target.rows()), sorted(&expected_rows(values)));
    }
    let last_statement = exact
        .checkpoints
        .last()
        .unwrap()
        .statement
        .as_ref()
        .unwrap();
    assert_read(last_statement, &case["expected_terminal_read"]);
    assert_terminal_domains(&exact, &case["typed_domains"]);

    if let Some(expected) = case["intermediate_read"].as_object() {
        let first = exact
            .checkpoints
            .iter()
            .filter_map(|checkpoint| checkpoint.statement.as_ref())
            .find(|statement| statement.read.is_some())
            .unwrap();
        assert_read(first, &Value::Object(expected.clone()));
    }
    if let Some(rolled_back) = case["checkpoint_t_after_rollback"].as_array() {
        let rollback = exact
            .checkpoints
            .iter()
            .find(|checkpoint| {
                checkpoint.action == sql_semantic_protocol::PhysicalTransactionAction::Rollback
            })
            .unwrap();
        let target = rollback
            .after
            .states
            .iter()
            .find(|state| state.relation() == "t")
            .unwrap();
        assert_eq!(
            sorted(target.rows()),
            sorted(&rolled_back.iter().map(row_from_json).collect::<Vec<_>>())
        );
    }
    if case["expected_same_catalog_after_both_merges"] == true {
        let updates = exact
            .checkpoints
            .iter()
            .filter_map(|checkpoint| checkpoint.statement.as_ref())
            .filter_map(|node| node.write.as_ref())
            .filter(|write| write.affected().updated() == 1)
            .collect::<Vec<_>>();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].after().rows(), updates[1].after().rows());
    }
}

#[test]
fn frozen_ddl_insert_merge_delete_select_script_matches_every_duckdb_checkpoint() {
    let freeze = frozen();
    run_native_case(&freeze["cases"][0]);
}

#[test]
fn frozen_duplicate_null_and_three_valued_rejection_bags_match_native_duckdb() {
    let freeze = frozen();
    run_native_case(&freeze["cases"][1]);
}

#[test]
fn frozen_rolled_back_merge_and_delete_restore_complete_original_rows() {
    let freeze = frozen();
    run_native_case(&freeze["cases"][2]);
}

#[test]
fn frozen_repeated_merge_is_idempotent_in_value_bag_but_counts_both_updates() {
    let freeze = frozen();
    run_native_case(&freeze["cases"][3]);
}

#[test]
fn native_alter_default_is_carried_across_independent_transaction_sql_units() {
    let freeze = frozen();
    let case = &freeze["cases"][7];
    let program = case["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step.as_str().unwrap())
        .collect::<Vec<_>>();
    let proof = physical_duckdb_sql_transaction_plan(
        &program,
        &PhysicalScriptCatalog {
            states: vec![],
            schemas: vec![],
        },
    );
    let PhysicalTransactionScriptProof::Exact(exact) = proof else {
        panic!("TASK-164 requires attested ALTER default carry: {proof:?}");
    };
    assert_eq!(exact.checkpoints.len(), program.len());
    let db = Connection::open_in_memory().unwrap();
    for (index, sql) in program.iter().enumerate() {
        if !sql.starts_with("SELECT") {
            db.execute_batch(sql).unwrap();
        }
        let native_count: i64 = db
            .query_row("SELECT count(*) FROM t", [], |result| result.get(0))
            .unwrap();
        assert_eq!(
            exact.checkpoints[index].after.states[0].rows().len(),
            native_count as usize
        );
        if index >= 2 {
            assert_eq!(exact.checkpoints[index].after.schemas[0].columns().len(), 2);
        }
    }
    assert_eq!(
        exact.final_catalog.states[0].rows(),
        &[row(Some(2), Some(5))]
    );
    let read = exact
        .checkpoints
        .last()
        .unwrap()
        .statement
        .as_ref()
        .unwrap();
    assert_read(read, &case["expected_terminal_read"]);
    assert_terminal_domains(&exact, &case["typed_domains"]);
    let from_native = native_rows(&db, "t");
    assert_eq!(
        sorted(&from_native),
        sorted(exact.final_catalog.states[0].rows())
    );
    let default_sources = exact
        .final_constraints
        .iter()
        .find(|set| set.relation() == "t")
        .unwrap()
        .column_value_sources();
    assert_eq!(default_sources.len(), 1);
    assert_eq!(default_sources[0].column(), "v");
}

#[test]
fn duplicate_create_is_impossible_and_native_duckdb_rejects_it() {
    let freeze = frozen();
    let case = &freeze["cases"][4];
    let program = case["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|step| step.as_str().unwrap())
        .collect::<Vec<_>>();
    let proof = physical_duckdb_sql_transaction_plan(
        &program,
        &PhysicalScriptCatalog {
            states: vec![],
            schemas: vec![],
        },
    );
    assert!(
        matches!(
            proof,
            PhysicalTransactionScriptProof::Impossible { step_index: 1, .. }
        ),
        "{proof:?}"
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(program[0]).unwrap();
    assert!(db.execute_batch(program[1]).is_err());
}

#[test]
fn unsupported_savepoints_and_vendor_ddl_laws_cannot_be_reported_exact() {
    let freeze = frozen();
    let empty = PhysicalScriptCatalog {
        states: vec![],
        schemas: vec![],
    };
    let savepoint = freeze["cases"][5]["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|sql| sql.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(matches!(
        physical_duckdb_sql_transaction_plan(&savepoint, &empty),
        PhysicalTransactionScriptProof::Residual { .. }
    ));
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("BEGIN").unwrap();
    assert!(db.execute_batch("SAVEPOINT x").is_err());

    let vendor = freeze["cases"][6]["program"]
        .as_array()
        .unwrap()
        .iter()
        .map(|sql| sql.as_str().unwrap())
        .collect::<Vec<_>>();
    let proof = physical_duckdb_sql_transaction_plan_with_laws(
        &vendor,
        &empty,
        &[PhysicalTransactionSessionLaw::VendorImplicitDdlCommit],
    );
    assert!(matches!(
        proof,
        PhysicalTransactionScriptProof::Residual { .. }
    ));
}

#[test]
fn frozen_inventory_retains_original_scope_and_explicit_unverified_signoff() {
    let freeze = frozen();
    let original: Value =
        serde_json::from_str(include_str!("../docs/task-164-acceptance-freeze-v1.json")).unwrap();
    assert_eq!(original["version"], 1);
    assert_eq!(freeze["version"], 2);
    assert_eq!(freeze["owner"], "TASK-164");
    assert_eq!(freeze["parent"], "TASK-84");
    assert_eq!(freeze["dialects"].as_array().unwrap().len(), 13);
    assert_eq!(freeze["cases"].as_array().unwrap().len(), 8);
    assert_eq!(
        freeze["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(
                |case| case["classification"] == "Exact" || case["classification"] == "Impossible"
            )
            .count(),
        6
    );
    let manifest: Value =
        serde_json::from_str(include_str!("../docs/coverage-manifest.json")).unwrap();
    assert_eq!(
        manifest["task_164_mixed_script_acceptance"]["freeze_version"],
        2
    );
    assert_eq!(
        manifest["task_164_mixed_script_acceptance"]["parent_closed"],
        false
    );
}
