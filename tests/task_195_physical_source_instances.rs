//! Independent TASK-195 DuckDB source-instance controls. SQL rowid is used
//! solely as a fixed native fixture identity, never as a portable SQL column
//! or proof that tied original ROW_NUMBER assigns stable physical identities.

use duckdb::Connection;

#[test]
fn joined_qualify_preserves_each_duplicate_physical_source_pair() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE orders(id INTEGER, amount INTEGER);
         INSERT INTO orders VALUES (1,10),(1,10),(2,20),(3,NULL),(NULL,10),(4,-5);
         CREATE TABLE customers(id INTEGER, amount INTEGER);
         INSERT INTO customers VALUES (1,10),(1,10),(2,100),(5,-5),(NULL,10)",
    )
    .expect("six source and five partner occurrences");

    let mut query = db
        .prepare(
            "SELECT o.rowid, c.rowid, o.id,
                    COUNT(*) OVER (PARTITION BY c.id) AS n
             FROM orders o JOIN customers c ON o.id = c.id",
        )
        .expect("native joined source instances");
    let mut all_pairs = query
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("complete join")
        .collect::<Result<Vec<_>, _>>()
        .expect("physical duplicate pair identities");
    all_pairs.sort();
    assert_eq!(
        all_pairs,
        vec![
            (0, 0, 1, 4),
            (0, 1, 1, 4),
            (1, 0, 1, 4),
            (1, 1, 1, 4),
            (2, 2, 2, 1)
        ],
        "each duplicate joins independently, rather than coalescing rows"
    );
    let selected = all_pairs.iter().filter(|(_, _, _, n)| *n > 1).count();
    let not_true = all_pairs.iter().filter(|(_, _, _, n)| *n <= 1).count();
    assert_eq!((selected, not_true), (4, 1));
    assert_eq!(selected + not_true, all_pairs.len());
}

#[test]
fn empty_source_global_and_rollup_group_results_have_different_cardinalities() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch("CREATE TABLE empty(id INTEGER, amount INTEGER)")
        .expect("typed but zero-row source");
    let mut global = db
        .prepare("SELECT COUNT(*) FROM empty")
        .expect("global aggregate");
    let count = global
        .query_row([], |row| row.get::<_, i64>(0))
        .expect("global empty aggregate has one row");
    assert_eq!(count, 0);

    let mut grouped = db
        .prepare("SELECT id, COUNT(*) FROM empty GROUP BY id")
        .expect("ordinary group");
    let grouped_rows = grouped
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("grouped empty output")
        .collect::<Result<Vec<_>, _>>()
        .expect("valid grouped rows");
    assert!(grouped_rows.is_empty(), "empty source has no regular group");

    let mut rollup = db
        .prepare("SELECT id, COUNT(*) FROM empty GROUP BY ROLLUP(id)")
        .expect("grand total");
    let rollup_rows = rollup
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("ROLLUP empty output")
        .collect::<Result<Vec<_>, _>>()
        .expect("valid ROLLUP rows");
    assert_eq!(rollup_rows, vec![(None, 0)]);

    let marker = db
        .prepare("SELECT GROUPING(id) FROM empty GROUP BY ROLLUP(id)")
        .expect("grouping provenance")
        .query_row([], |row| row.get::<_, i64>(0))
        .expect("grand total subtotal marker");
    assert_eq!(
        marker, 1,
        "subtotal must not be confused with physical NULL"
    );
}

#[test]
fn null_sort_precedence_is_known_but_peer_identity_is_not_total_order() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE orders(id INTEGER, amount INTEGER);
         INSERT INTO orders VALUES (1,10),(1,10),(2,20),(3,NULL),(NULL,10),(4,-5)",
    )
    .expect("fixed source");
    let mut statement = db
        .prepare("SELECT id, amount FROM orders ORDER BY amount NULLS FIRST")
        .expect("native explicit NULL order");
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?))
        })
        .expect("complete ordered output")
        .collect::<Result<Vec<_>, _>>()
        .expect("all ordered values");
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0], (Some(3), None), "NULL amount FIRST");
    assert_eq!(rows[1], (Some(4), Some(-5)));
    assert_eq!(rows[5], (Some(2), Some(20)));
    let mut peer_group = rows[2..5].to_vec();
    peer_group.sort();
    assert_eq!(
        peer_group,
        vec![(None, Some(10)), (Some(1), Some(10)), (Some(1), Some(10))],
        "peer tuple bag is exact but order of equal sort keys is not"
    );
}
