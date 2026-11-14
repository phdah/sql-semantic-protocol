//! Independent DuckDB terminal-bag oracles for TASK-194's original set SQL.
//! Counts are compared as full positional tuples, never independent columns.

use duckdb::Connection;
use serde_json::Value;
use std::collections::BTreeSet;

const FREEZE: &str = include_str!("../docs/task-194-acceptance-freeze.json");

fn freeze() -> Value {
    serde_json::from_str(FREEZE).expect("task-owned, immutable acceptance fixture")
}

fn expected_rows(value: &Value) -> Vec<Vec<Option<i64>>> {
    let mut rows = value
        .as_array()
        .expect("literal terminal bag")
        .iter()
        .map(|tuple| {
            tuple
                .as_array()
                .expect("complete positional tuple")
                .iter()
                .map(Value::as_i64)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn query_rows(db: &Connection, sql: &str, arity: usize) -> Vec<Vec<Option<i64>>> {
    let mut stmt = db.prepare(sql).expect("independent native SQL parser");
    let mut rows = stmt
        .query_map([], |row| {
            let mut tuple = Vec::with_capacity(arity);
            for index in 0..arity {
                tuple.push(row.get::<_, Option<i64>>(index)?);
            }
            Ok(tuple)
        })
        .expect("independent DuckDB terminal execution")
        .collect::<Result<Vec<_>, _>>()
        .expect("all duplicate occurrences and SQL NULL values");
    rows.sort();
    rows
}

#[test]
fn twenty_original_executable_set_queries_match_complete_duckdb_terminal_bags() {
    let fixture = freeze();
    assert_eq!(fixture["case_count"], 22);
    assert_eq!(fixture["parser_cells"], 286);
    let db = Connection::open_in_memory().expect("native DuckDB");
    db.execute_batch(
        fixture["source_setup"]
            .as_str()
            .expect("authoritative closed-world seed"),
    )
    .expect("seed the two complete source tables");
    for source in ["orders", "customers"] {
        let source_sql = format!("SELECT id, amount FROM {source}");
        let physical = query_rows(&db, &source_sql, 2);
        let expected = expected_rows(&fixture["complete_source_bags"][source]);
        assert_eq!(
            physical, expected,
            "{source}: complete source bag with duplicates"
        );
    }

    let mut checked = BTreeSet::new();
    let mut executed = 0;
    let mut total_output_occurrences = 0;
    for case in fixture["cases"].as_array().expect("all 22 cases") {
        let id = case["id"].as_str().expect("stable original ID");
        assert!(checked.insert(id), "{id}: duplicate original SQL");
        let Some(sql) = case["native_duckdb_sql"].as_str() else {
            assert!(
                id == "88-C177-S084" || id == "88-C177-S086",
                "{id}: unowned native omission"
            );
            assert!(case["expected_terminal_bag"].is_null(), "{id}");
            continue;
        };
        assert_eq!(
            case["sql"], sql,
            "{id}: native oracle must execute ORIGINAL SQL"
        );
        let arity = case["expected_output_columns"].as_u64().expect("arity") as usize;
        let expected = expected_rows(&case["expected_terminal_bag"]);
        let observed = query_rows(&db, sql, arity);
        assert_eq!(observed, expected, "{id}: complete positional multiset");
        assert!(observed.iter().all(|row| row.len() == arity), "{id}: arity");
        total_output_occurrences += observed.len();
        executed += 1;
    }
    assert_eq!(checked.len(), 22);
    assert_eq!(executed, 20);
    assert!(
        total_output_occurrences > 100,
        "all duplicate occurrences verified"
    );
}

#[test]
fn tuple_multiplicity_and_null_equality_are_not_set_membership_shortcuts() {
    let fixture = freeze();
    let cases = fixture["cases"].as_array().expect("22 named SQL shapes");
    let terminal = |id: &str| {
        expected_rows(
            &cases
                .iter()
                .find(|case| case["id"] == id)
                .expect("original named SQL")["expected_terminal_bag"],
        )
    };
    assert_eq!(
        terminal("88-C177-S075"),
        vec![vec![None], vec![Some(1)], vec![Some(1)], vec![Some(2)]],
        "INTERSECT ALL keeps min(2,2) copies of 1 and one common NULL"
    );
    assert_eq!(
        terminal("88-C177-S079"),
        vec![vec![Some(3)], vec![Some(4)]],
        "EXCEPT ALL subtracts every matching physical occurrence"
    );
    assert_eq!(
        terminal("88-C177-S070"),
        vec![
            vec![None],
            vec![Some(1)],
            vec![Some(2)],
            vec![Some(3)],
            vec![Some(4)],
            vec![Some(5)]
        ],
        "UNION DISTINCT collapses NULL and repeated source tuple values"
    );
    assert_eq!(
        terminal("88-C177-S082"),
        vec![vec![Some(5)]],
        "EXCEPT is directional, not a commutative set operation"
    );
    let by_name = terminal("88-C177-S083");
    let positional = terminal("88-C177-S085");
    assert_ne!(by_name, positional, "BY NAME must align by column identity");
    assert!(by_name.contains(&vec![Some(5), Some(-5)]));
    assert!(positional.contains(&vec![Some(-5), Some(5)]));
    assert_eq!(
        terminal("88-C177-S087").len(),
        17,
        "two orders bags plus customers"
    );
}
