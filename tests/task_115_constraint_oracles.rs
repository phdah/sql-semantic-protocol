//! Independent physical PK/FK law oracle for TASK-115.
use duckdb::Connection;

#[test]
fn enforced_keys_allow_duplicate_join_values_but_not_invalid_physical_rows() {
    let db = Connection::open_in_memory().expect("DuckDB");
    db.execute_batch(
        "CREATE TABLE t(tid INTEGER PRIMARY KEY, k INTEGER);
         CREATE TABLE r(id VARCHAR PRIMARY KEY, tid INTEGER REFERENCES t(tid), k INTEGER);
         CREATE TABLE l(id VARCHAR PRIMARY KEY, k INTEGER, a INTEGER);
         INSERT INTO t VALUES (1,1);
         INSERT INTO r VALUES ('r1',1,1),('r2',1,1);
         INSERT INTO l VALUES ('l1',1,1),('l2',1,2),('l3',2,1);"
    ).expect("complete constrained sources");

    let identities: String = db.query_row(
        "SELECT string_agg(l.id || ':' || r.id || ':' || t.tid::VARCHAR,
                           ',' ORDER BY l.id,r.id,t.tid)
         FROM l JOIN r ON l.k=r.k JOIN t ON r.tid=t.tid
         WHERE l.a>0",
        [], |row| row.get(0),
    ).expect("full constrained join bag");
    assert_eq!(identities, "l1:r1:1,l1:r2:1,l2:r1:1,l2:r2:1");

    let rejected: i64 = db.query_row(
        "SELECT COUNT(*) FROM l CROSS JOIN r CROSS JOIN t
         WHERE (l.a>0 AND l.k=r.k AND r.tid=t.tid) IS NOT TRUE",
        [], |row| row.get(0),
    ).expect("closed NOT TRUE complement");
    assert_eq!(rejected, 2);

    assert!(
        db.execute("INSERT INTO l VALUES ('l1',3,1)", []).is_err(),
        "duplicate primary-key identity cannot be a physical source row"
    );
    assert!(
        db.execute("INSERT INTO r VALUES ('r3',99,1)", []).is_err(),
        "unmatched FK cannot be assumed insertable"
    );
}
