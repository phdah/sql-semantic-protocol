//! TASK-114 independent DuckDB source/terminal oracle baseline.
//! A passing SQL engine is not a constructive canonical protocol proof.
use duckdb::Connection;
use serde_json::json;

fn rows(db: &Connection, sql: &str) -> Vec<String> {
    let mut statement = db.prepare(sql).unwrap();
    let mut output = statement
        .query_map([], |row| {
            let left: String = row.get(0)?;
            let right: Option<String> = row.get(1)?;
            Ok(json!([left, right]).to_string())
        })
        .unwrap()
        .map(|row| row.unwrap())
        .collect::<Vec<_>>();
    output.sort();
    output
}

fn expected(pairs: &[(&str, Option<&str>)]) -> Vec<String> {
    let mut result = pairs
        .iter()
        .map(|(l, r)| json!([l, r]).to_string())
        .collect::<Vec<_>>();
    result.sort();
    result
}

#[test]
fn nearest_prior_asof_chooses_one_right_row_not_all_eligible_pairs() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, t INTEGER);         CREATE TABLE r(id VARCHAR, t INTEGER);         INSERT INTO l VALUES ('l3',3), ('l5',5), ('l10',10), ('ln',NULL);         INSERT INTO r VALUES ('r4',4), ('r7',7), ('rn',NULL);",
    )
    .unwrap();

    let selected = rows(&db, "SELECT l.id, r.id FROM l ASOF JOIN r ON l.t >= r.t");
    assert_eq!(
        selected,
        expected(&[("l5", Some("r4")), ("l10", Some("r7"))])
    );

    let null_extended = rows(
        &db,
        "SELECT l.id, r.id FROM l ASOF LEFT JOIN r ON l.t >= r.t",
    );
    assert_eq!(
        null_extended,
        expected(&[
            ("l3", None),
            ("l5", Some("r4")),
            ("l10", Some("r7")),
            ("ln", None),
        ])
    );

    // An ordinary inequality join also includes (l10,r4). ASOF must not.
    let eligible = rows(
        &db,
        "SELECT l.id, r.id FROM l CROSS JOIN r WHERE l.t >= r.t",
    );
    assert_eq!(
        eligible,
        expected(&[("l5", Some("r4")), ("l10", Some("r4")), ("l10", Some("r7")),])
    );
    assert_eq!(selected.len(), 2);
    assert_eq!(eligible.len(), 3);
    assert_eq!(
        rows(
            &db,
            "SELECT l.id, r.id FROM l CROSS JOIN r WHERE (l.t >= r.t) IS NOT TRUE",
        )
        .len(),
        9,
        "NULL and rejected physical pairs must be accounted for"
    );
}

#[test]
fn correlated_lateral_evaluates_each_left_row_and_preserves_duplicates() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, t INTEGER);         CREATE TABLE r(id VARCHAR, t INTEGER);         INSERT INTO l VALUES ('l1',1), ('l2',2), ('ln',NULL);         INSERT INTO r VALUES ('r1',1), ('r1b',1), ('r2',2), ('rn',NULL);",
    )
    .unwrap();

    let lateral = "(SELECT r.id FROM r WHERE r.t = l.t) q";
    let inner = rows(
        &db,
        &format!("SELECT l.id, q.id FROM l CROSS JOIN LATERAL {lateral}"),
    );
    assert_eq!(
        inner,
        expected(&[("l1", Some("r1")), ("l1", Some("r1b")), ("l2", Some("r2")),])
    );

    let outer = rows(
        &db,
        &format!("SELECT l.id, q.id FROM l LEFT JOIN LATERAL {lateral} ON TRUE"),
    );
    assert_eq!(
        outer,
        expected(&[
            ("l1", Some("r1")),
            ("l1", Some("r1b")),
            ("l2", Some("r2")),
            ("ln", None),
        ])
    );
    assert_eq!(
        rows(
            &db,
            "SELECT l.id, r.id FROM l CROSS JOIN r WHERE (l.t = r.t) IS NOT TRUE",
        )
        .len(),
        9,
        "complete 3 x 4 input pairs must partition into 3 TRUE and 9 NOT TRUE"
    );
}

#[test]
fn inequality_range_join_proves_both_selected_and_not_true_bags() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, t INTEGER);         CREATE TABLE r(id VARCHAR, t INTEGER);         INSERT INTO l VALUES ('l5',5), ('l10',10), ('ln',NULL);         INSERT INTO r VALUES ('r4',4), ('r7',7), ('rn',NULL);",
    )
    .unwrap();

    assert_eq!(
        rows(&db, "SELECT l.id, r.id FROM l JOIN r ON l.t > r.t"),
        expected(&[("l5", Some("r4")), ("l10", Some("r4")), ("l10", Some("r7")),])
    );
    assert_eq!(
        rows(
            &db,
            "SELECT l.id, r.id FROM l CROSS JOIN r WHERE (l.t > r.t) IS NOT TRUE",
        ),
        expected(&[
            ("l5", Some("r7")),
            ("l5", Some("rn")),
            ("l10", Some("rn")),
            ("ln", Some("r4")),
            ("ln", Some("r7")),
            ("ln", Some("rn")),
        ])
    );
}

#[test]
fn timestamp_inequality_includes_equal_instants_and_rejects_unknown() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE l(id VARCHAR, t TIMESTAMP);
         CREATE TABLE r(id VARCHAR, t TIMESTAMP);
         INSERT INTO l VALUES ('le', TIMESTAMP '2026-10-17 00:05:00'), ('ln',NULL);
         INSERT INTO r VALUES ('re', TIMESTAMP '2026-10-17 00:05:00'), ('rn',NULL);",
    )
    .unwrap();
    assert_eq!(
        rows(&db, "SELECT l.id,r.id FROM l JOIN r ON l.t >= r.t"),
        expected(&[("le", Some("re"))]),
        "inclusive timestamp lower comparison must retain equality"
    );
    assert_eq!(
        rows(
            &db,
            "SELECT l.id,r.id FROM l CROSS JOIN r WHERE (l.t >= r.t) IS NOT TRUE"
        ),
        expected(&[("le", Some("rn")), ("ln", Some("re")), ("ln", Some("rn")),]),
        "all three UNKNOWN physical pairs are nonmatches"
    );
}
