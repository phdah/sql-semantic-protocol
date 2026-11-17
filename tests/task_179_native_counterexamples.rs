//! TASK-179 independent executable DuckDB counterexamples for vendor/session laws.
//!
//! An engine-local outcome is not a statement about Snowflake/PostgreSQL or
//! any other dialect accepted by sqlparser. Each source bag and rejected
//! occurrence is asserted independently from the native SQL result.

use duckdb::Connection;

fn nullable_ids(db: &Connection, sql: &str) -> Vec<Option<i64>> {
    let mut stmt = db.prepare(sql).expect("native bounded SELECT");
    stmt.query_map([], |row| row.get(0))
        .expect("DuckDB rows")
        .map(|row| row.expect("nullable row ID"))
        .collect()
}

fn source_pairs(db: &Connection, sql: &str) -> Vec<(i64, Option<String>)> {
    let mut stmt = db.prepare(sql).expect("complete typed source");
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("native full source rows")
        .map(|row| row.expect("source row"))
        .collect()
}

#[test]
fn native_like_ilike_padding_collation_and_not_true_do_not_share_one_law() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE names(row_id INTEGER, txt VARCHAR);
         INSERT INTO names VALUES
         (1,'abc'),(2,'ABC'),(3,'abc '),(4,NULL),(5,'abc');",
    )
    .expect("complete string bag with duplicate value, case, padding and NULL");
    assert_eq!(
        source_pairs(&db, "SELECT row_id, txt FROM names ORDER BY row_id"),
        vec![
            (1, Some("abc".to_string())),
            (2, Some("ABC".to_string())),
            (3, Some("abc ".to_string())),
            (4, None),
            (5, Some("abc".to_string())),
        ]
    );
    assert_eq!(
        nullable_ids(&db, "SELECT row_id FROM names WHERE txt LIKE 'a%' ORDER BY row_id"),
        vec![Some(1), Some(3), Some(5)]
    );
    assert_eq!(
        nullable_ids(&db, "SELECT row_id FROM names WHERE txt ILIKE 'a%' ORDER BY row_id"),
        vec![Some(1), Some(2), Some(3), Some(5)]
    );
    assert_eq!(
        nullable_ids(&db, "SELECT row_id FROM names WHERE txt = 'abc' ORDER BY row_id"),
        vec![Some(1), Some(5)]
    );
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM names WHERE txt COLLATE NOCASE = 'abc' ORDER BY row_id"
        ),
        vec![Some(1), Some(2), Some(5)]
    );
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM names WHERE (txt = 'abc') IS NOT TRUE ORDER BY row_id"
        ),
        vec![Some(2), Some(3), Some(4)],
        "negative partition includes the NULL source occurrence"
    );
}

#[test]
fn session_default_null_order_reverses_only_tie_deterministic_order() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE amounts(row_id INTEGER, amount INTEGER);
         INSERT INTO amounts VALUES (1,NULL),(2,0),(3,0),(4,2);",
    )
    .expect("duplicate plus nullable integer population");
    assert_eq!(
        source_pairs(
            &db,
            "SELECT row_id, CAST(amount AS VARCHAR) FROM amounts ORDER BY row_id"
        ),
        vec![
            (1, None),
            (2, Some("0".to_string())),
            (3, Some("0".to_string())),
            (4, Some("2".to_string())),
        ]
    );
    db.execute_batch("SET default_null_order = 'NULLS_FIRST'")
        .expect("DuckDB session NULL ordering");
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM amounts ORDER BY amount ASC, row_id ASC"
        ),
        vec![Some(1), Some(2), Some(3), Some(4)]
    );
    db.execute_batch("SET default_null_order = 'NULLS_LAST'")
        .expect("alternate DuckDB session NULL ordering");
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM amounts ORDER BY amount ASC, row_id ASC"
        ),
        vec![Some(2), Some(3), Some(4), Some(1)]
    );
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM amounts ORDER BY amount ASC NULLS FIRST, row_id ASC"
        ),
        vec![Some(1), Some(2), Some(3), Some(4)],
        "explicit SQL NULL ordering overrides the session default"
    );
}

#[test]
fn malformed_implicit_coercion_fails_where_try_cast_stays_nullable() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE amounts(row_id INTEGER, amount INTEGER);
         INSERT INTO amounts VALUES (1,2),(2,NULL),(3,10);",
    )
    .expect("closed integer source");
    assert_eq!(
        source_pairs(
            &db,
            "SELECT row_id, CAST(amount AS VARCHAR) FROM amounts ORDER BY row_id"
        ),
        vec![
            (1, Some("2".to_string())),
            (2, None),
            (3, Some("10".to_string())),
        ]
    );
    let native_error = db.prepare("SELECT row_id FROM amounts WHERE amount = 'invalid'")
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, i64>(0))?
                .collect::<duckdb::Result<Vec<_>>>()
        });
    assert!(
        native_error.is_err(),
        "native implicit cast failure must never be reported as empty exact output"
    );
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM amounts WHERE amount = TRY_CAST('invalid' AS INTEGER)"
        ),
        Vec::<Option<i64>>::new()
    );
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM amounts WHERE (amount = TRY_CAST('invalid' AS INTEGER)) IS NOT TRUE ORDER BY row_id"
        ),
        vec![Some(1), Some(2), Some(3)],
        "TRY_CAST NULL comparison is UNKNOWN for every source row"
    );
}

#[test]
fn finite_merge_retains_complete_unmodified_and_inserted_target_bag() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE target(row_id INTEGER, amount INTEGER);
         CREATE TABLE source(row_id INTEGER, amount INTEGER);
         INSERT INTO target VALUES (1,10),(2,NULL);
         INSERT INTO source VALUES (1,20),(3,30);",
    )
    .expect("closed target and source physical bags");
    assert_eq!(
        source_pairs(
            &db,
            "SELECT row_id, CAST(amount AS VARCHAR) FROM target ORDER BY row_id"
        ),
        vec![(1, Some("10".to_string())), (2, None)]
    );
    assert_eq!(
        source_pairs(
            &db,
            "SELECT row_id, CAST(amount AS VARCHAR) FROM source ORDER BY row_id"
        ),
        vec![
            (1, Some("20".to_string())),
            (3, Some("30".to_string())),
        ]
    );
    db.execute_batch(
        "MERGE INTO target AS t USING source AS s ON t.row_id = s.row_id
         WHEN MATCHED THEN UPDATE SET amount = s.amount
         WHEN NOT MATCHED THEN INSERT (row_id, amount) VALUES (s.row_id, s.amount);",
    )
    .expect("DuckDB closed one-to-one MERGE");
    assert_eq!(
        source_pairs(
            &db,
            "SELECT row_id, CAST(amount AS VARCHAR) FROM target ORDER BY row_id"
        ),
        vec![
            (1, Some("20".to_string())),
            (2, None),
            (3, Some("30".to_string())),
        ],
        "target 2 remains untouched; matched update and unmatched insert are separate effects"
    );
}

#[test]
fn duckdb_ddl_rollback_restores_catalog_without_claiming_vendor_autocommit() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch("CREATE TABLE base(row_id INTEGER); INSERT INTO base VALUES (1),(1);")
        .expect("physical source outside transaction");
    assert_eq!(
        nullable_ids(&db, "SELECT row_id FROM base ORDER BY rowid"),
        vec![Some(1), Some(1)]
    );
    db.execute_batch(
        "BEGIN;
         CREATE TABLE transient(row_id INTEGER);
         INSERT INTO transient VALUES (2),(NULL);
         ROLLBACK;",
    )
    .expect("single-connection DuckDB DDL rollback");
    assert_eq!(
        nullable_ids(&db, "SELECT row_id FROM base ORDER BY rowid"),
        vec![Some(1), Some(1)],
        "rollback cannot erase unrelated committed duplicate source rows"
    );
    let transient_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_name='transient'",
            [],
            |row| row.get(0),
        )
        .expect("native information schema");
    assert_eq!(transient_count, 0);
    assert!(db.prepare("SELECT * FROM transient").is_err());
}

#[test]
fn native_identifier_folding_prevents_quoted_case_collision() {
    let db = Connection::open_in_memory().expect("DuckDB");
    assert!(
        db.execute_batch("CREATE TABLE \"CaseSensitive\"(\"Value\" INT, \"value\" INT)")
            .is_err(),
        "DuckDB quoted names cannot prove distinct columns by case alone"
    );
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_name='CaseSensitive'",
            [],
            |row| row.get(0),
        )
        .expect("catalog state after failed DDL");
    assert_eq!(count, 0, "failed DDL must not leak a partial relation");
}

#[test]
fn nan_and_null_are_distinct_native_double_source_cases() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE floats(row_id INTEGER, amount DOUBLE);
         INSERT INTO floats VALUES (1, CAST('NaN' AS DOUBLE)),(2,0.0),(3,NULL);",
    )
    .expect("native NaN and NULL source distinctions");
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM floats WHERE isnan(amount) ORDER BY row_id"
        ),
        vec![Some(1)]
    );
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM floats WHERE amount = 0.0 ORDER BY row_id"
        ),
        vec![Some(2)]
    );
    assert_eq!(
        nullable_ids(
            &db,
            "SELECT row_id FROM floats WHERE (amount = 0.0) IS NOT TRUE ORDER BY row_id"
        ),
        vec![Some(1), Some(3)],
        "NaN is a native nonzero value; NULL is still UNKNOWN"
    );
}
