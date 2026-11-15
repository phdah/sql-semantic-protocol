//! TASK-195 original SQL DuckDB window, grouping and pagination evidence.
//! Literal bag goldens are independent of parser output and preserve SQL NULL,
//! duplicate occurrences, peer ranks and opaque engine/session boundaries.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::Value;

const FREEZE: &str = include_str!("../docs/task-195-acceptance-freeze.json");

fn freeze() -> Value {
    serde_json::from_str(FREEZE).expect("versioned task-owned oracle")
}

fn source_rows(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut tuples = value
        .as_array()
        .expect("literal complete multiset")
        .iter()
        .map(|row| {
            row.as_array()
                .expect("tuple")
                .iter()
                .map(Value::as_i64)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    tuples.sort();
    tuples
}

fn original_integer_rows(
    db: &Connection,
    sql: &str,
    columns: usize,
) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("original native SQL");
    stmt.query_map([], |row| {
        let mut tuple = Vec::with_capacity(columns);
        for index in 0..columns {
            tuple.push(row.get::<_, Option<i64>>(index)?);
        }
        Ok(tuple)
    })
    .expect("native integer query")
    .collect::<Result<Vec<_>, _>>()
    .expect("complete source/terminal occurrences")
}

fn original_float_rows(db: &Connection, sql: &str) -> Vec<Option<f64>> {
    let mut stmt = db.prepare(sql).expect("original native float SQL");
    stmt.query_map([], |row| row.get::<_, Option<f64>>(0))
        .expect("native floating-point query")
        .collect::<Result<Vec<_>, _>>()
        .expect("complete native float occurrences")
}

fn float_cmp(left: &Option<f64>, right: &Option<f64>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(x), Some(y)) => x.total_cmp(y),
    }
}

fn verify_float_bag(db: &Connection, sql: &str, expected: &Value, id: &str) {
    let mut observed = original_float_rows(db, sql);
    let mut golden = expected
        .as_array()
        .expect("expected numeric terminal bag")
        .iter()
        .map(|row| {
            row.as_array()
                .expect("single column tuple")[0]
                .as_f64()
        })
        .collect::<Vec<_>>();
    observed.sort_by(float_cmp);
    golden.sort_by(float_cmp);
    assert_eq!(observed.len(), golden.len(), "{id}: entire terminal bag");
    for (index, (actual, expected)) in observed.iter().zip(&golden).enumerate() {
        match (actual, expected) {
            (None, None) => {}
            (Some(a), Some(e)) => assert!(
                (a - e).abs() < 1e-10,
                "{id}: numeric result at row {index}: {a} != {e}"
            ),
            _ => panic!("{id}: NULL and real number are not interchangeable"),
        }
    }
}

fn find_case<'a>(fixture: &'a Value, id: &str) -> &'a Value {
    fixture["cases"]
        .as_array()
        .expect("36 original samples")
        .iter()
        .find(|case| case["id"] == id)
        .expect("named TASK-195 sample")
}

#[test]
fn original_duckdb_sql_preserves_complete_native_source_and_terminal_bags() {
    let fixture = freeze();
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(fixture["source_setup"].as_str().expect("closed source DDL"))
        .expect("seed actual duplicate and NULL source occurrences");
    for (source, arity) in [("orders", 2), ("customers", 2), ("products", 1)] {
        let sql = format!("SELECT * FROM {source}");
        let mut actual = original_integer_rows(&db, &sql, arity);
        actual.sort();
        assert_eq!(
            actual,
            source_rows(&fixture["complete_source_bags"][source]),
            "{source}: complete and independently fixed physical source"
        );
    }

    let mut checked = BTreeSet::new();
    let mut verified = 0;
    let mut parser_boundaries = 0;
    let mut stochastic = 0;
    let mut unstable_tie = 0;
    for case in fixture["cases"].as_array().expect("36 original SQL shapes") {
        let id = case["id"].as_str().expect("stable ID");
        assert!(checked.insert(id), "{id}: duplicate original SQL");
        let sql = case["sql"].as_str().expect("immutable original spelling");
        let native = &case["native_oracle"];
        match native["kind"].as_str().expect("explicit proof class") {
            "prior_original_integer_bag" | "original_integer_bag" | "unordered_original_source_bag" => {
                let arity = native["columns"].as_u64().expect("fixed output arity") as usize;
                let mut actual = original_integer_rows(&db, sql, arity);
                actual.sort();
                assert_eq!(
                    actual,
                    source_rows(&native["expected_terminal_bag"]),
                    "{id}: original native terminal multiset including NULL and peers"
                );
                verified += 1;
            }
            "original_float_bag" => {
                verify_float_bag(&db, sql, &native["expected_terminal_bag"], id);
                verified += 1;
            }
            "unstable_tie_membership" => {
                let actual = original_integer_rows(&db, sql, 1);
                assert_eq!(actual.len(), 2, "{id}: LIMIT count");
                assert_eq!(actual[0], vec![Some(4)], "{id}: strictly first sort key");
                assert!(
                    actual[1] == vec![Some(1)] || actual[1] == vec![None],
                    "{id}: a peer-selected result may not invent an absent source row"
                );
                assert!(native["expected_terminal_bag"].is_null());
                assert_eq!(native["boundary_owner"], "TASK-179");
                unstable_tie += 1;
            }
            "native_parser_rejection" => {
                assert!(
                    db.prepare(sql).is_err(),
                    "{id}: rejected original FETCH WITH TIES / TOP cannot be called executed"
                );
                assert!(native["expected_terminal_bag"].is_null());
                assert_eq!(native["boundary_owner"], "TASK-191");
                parser_boundaries += 1;
            }
            "stochastic_conditional" => {
                // No deterministic bag can be frozen from BERNOULLI / SYSTEM.
                // Execution and seed/session vendor-law proof belongs to TASK-179/191.
                assert!(sql.contains("TABLESAMPLE"), "{id}: random native boundary");
                assert!(native["expected_terminal_bag"].is_null());
                assert_eq!(native["boundary_owner"], "TASK-179/TASK-191");
                stochastic += 1;
            }
            other => panic!("{id}: missing implementation for native law {other}"),
        }
    }
    assert_eq!(checked.len(), 36);
    assert_eq!(verified, 28);
    assert_eq!(unstable_tie, 1);
    assert_eq!(parser_boundaries, 2);
    assert_eq!(stochastic, 5);
}

#[test]
fn physical_null_group_and_generated_subtotal_are_not_equivalent() {
    let fixture = freeze();
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(fixture["source_setup"].as_str().expect("physical input"))
        .expect("source seeded");

    let rollup = find_case(&fixture, "88-C177-S100");
    let grouping = find_case(&fixture, "88-C177-S104");
    let mut rolls = original_integer_rows(&db, rollup["sql"].as_str().expect("ROLLUP"), 2);
    rolls.sort();
    assert!(rolls.contains(&vec![None, Some(1)]), "physical NULL id group");
    assert!(rolls.contains(&vec![None, Some(6)]), "generated subtotal for all rows");

    let mut actual = original_integer_rows(&db, grouping["sql"].as_str().expect("GROUPING"), 2);
    actual.sort();
    assert_eq!(
        actual.iter().filter(|row| row[0] == Some(1)).count(),
        1,
        "only generated subtotal has GROUPING(id)=1"
    );
    assert!(actual.contains(&vec![Some(0), Some(1)]), "physical NULL remains a real group");
    assert!(actual.contains(&vec![Some(1), Some(6)]), "all six source occurrences retained");
}

#[test]
fn ranking_with_peers_and_fetch_with_ties_differ_from_row_limits() {
    let fixture = freeze();
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(fixture["source_setup"].as_str().expect("physical input"))
        .expect("source seeded");

    for (id, golden) in [
        ("88-C177-S111", vec![1, 2, 2, 2, 5, 6]),
        ("88-C177-S112", vec![1, 2, 2, 2, 3, 4]),
    ] {
        let case = find_case(&fixture, id);
        let mut observed = original_integer_rows(&db, case["sql"].as_str().expect("RANK"), 1);
        observed.sort();
        assert_eq!(
            observed,
            golden.into_iter().map(|n| vec![Some(n)]).collect::<Vec<_>>(),
            "{id}: RANK gaps and DENSE_RANK peers are different"
        );
    }

    // This *companion* query proves the peer law, not execution of S160.
    let one_row = original_integer_rows(&db, "SELECT id FROM orders ORDER BY id LIMIT 1", 1);
    let tied_rows = original_integer_rows(
        &db,
        "SELECT id FROM (SELECT id, DENSE_RANK() OVER (ORDER BY id) AS rank_id FROM orders) r WHERE rank_id = 1",
        1,
    );
    assert_eq!(one_row, vec![vec![Some(1)]]);
    assert_eq!(tied_rows, vec![vec![Some(1)], vec![Some(1)]]);
    assert_eq!(
        find_case(&fixture, "88-C177-S160")["native_oracle"]["kind"],
        "native_parser_rejection",
        "companion must not be misreported as original FETCH WITH TIES proof"
    );
}
