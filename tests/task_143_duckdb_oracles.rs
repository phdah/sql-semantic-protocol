//! Native DuckDB full source, producer and terminal bags for the TASK-143 fixtures.

use duckdb::{params, Connection};

const SQL: &str = "WITH stage AS (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1) \
SELECT a.shifted AS l, b.shifted AS r FROM stage a \
JOIN stage b ON a.k = b.k ORDER BY l, r";

fn execute(rows: &[Option<i64>]) -> (Vec<Option<i64>>, Vec<(i64, i64)>, Vec<(i64, i64)>) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE raw (k INTEGER)").unwrap();
    for row in rows {
        db.execute("INSERT INTO raw VALUES (?1)", params![row]).unwrap();
    }
    let physical = db
        .prepare("SELECT k FROM raw ORDER BY k NULLS LAST")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<duckdb::Result<Vec<Option<i64>>>>()
        .unwrap();
    let stage = db
        .prepare("WITH stage AS (SELECT k, k + 10 AS shifted FROM raw WHERE k > 1) SELECT k, shifted FROM stage ORDER BY k, shifted")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<(i64, i64)>>>()
        .unwrap();
    let terminal = db
        .prepare(SQL)
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<duckdb::Result<Vec<(i64, i64)>>>()
        .unwrap();
    (physical, stage, terminal)
}

#[test]
fn positive_cte_filter_and_two_consumer_join_match_native_complete_bags() {
    let (physical, stage, terminal) = execute(&[Some(1), Some(2), Some(3)]);
    assert_eq!(physical, vec![Some(1), Some(2), Some(3)]);
    assert_eq!(stage, vec![(2, 12), (3, 13)]);
    assert_eq!(terminal, vec![(12, 12), (13, 13)]);
}

#[test]
fn null_duplicate_and_rejected_source_values_match_native_bags() {
    let (physical, stage, terminal) = execute(&[Some(1), Some(2), Some(2), None]);
    assert_eq!(physical, vec![Some(1), Some(2), Some(2), None]);
    assert_eq!(stage, vec![(2, 12), (2, 12)]);
    assert_eq!(terminal, vec![(12, 12); 4]);
}
