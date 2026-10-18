//! Independent execution of all versioned TASK-115 source/result bag oracles.
//! These assertions protect the acceptance inventory; they do not claim the
//! protocol can yet construct every mandatory variant.
use duckdb::Connection;
use serde_json::Value;

fn quote_sql(value: &Value) -> String {
    if value.is_null() {
        "NULL".to_string()
    } else if let Some(text) = value.as_str() {
        format!("'{}'", text.replace('\'', "''"))
    } else {
        value.to_string()
    }
}

fn insert_rows(db: &Connection, name: &str, rows: &[Value]) {
    for row in rows {
        let sql = match name {
            "l" => format!(
                "INSERT INTO l(id,k,k2,a) VALUES ({},{},{},{})",
                quote_sql(&row["id"]), quote_sql(&row["k"]),
                quote_sql(&row["k2"]), quote_sql(&row["a"])
            ),
            "r" => format!(
                "INSERT INTO r(id,k,k2,tid) VALUES ({},{},{},{})",
                quote_sql(&row["id"]), quote_sql(&row["k"]),
                quote_sql(&row["k2"]), quote_sql(&row["tid"])
            ),
            "t" => format!(
                "INSERT INTO t(id,k,k2,tid) VALUES ({},{},{},{})",
                quote_sql(&row["id"]), quote_sql(&row["k"]),
                quote_sql(&row["k2"]), quote_sql(&row["tid"])
            ),
            _ => panic!("unknown source"),
        };
        db.execute(&sql, []).expect("insert source row");
    }
}

fn identities(value: &Value) -> Vec<String> {
    let mut rows = value.as_array().expect("fixture rows")
        .iter().map(|triple| {
            let parts = triple.as_array().expect("triple identity");
            parts.iter().map(|v| v.as_str().expect("source row id"))
                .collect::<Vec<_>>().join(":")
        }).collect::<Vec<_>>();
    rows.sort();
    rows
}

fn actual_identity_bag(db: &Connection, truth: &str, select_true: bool) -> Vec<String> {
    let comparison = if select_true { "TRUE" } else { "NOT TRUE" };
    let sql = format!(
        "SELECT l.id || ':' || r.id || ':' || t.id
         FROM l CROSS JOIN r CROSS JOIN t
         WHERE ({truth}) IS {comparison}"
    );
    let mut statement = db.prepare(&sql).expect("identity cross-bag oracle");
    let results = statement.query_map([], |row| row.get::<_, String>(0))
        .expect("source identity bag");
    let mut bag = results.map(|result| result.expect("row identity")).collect::<Vec<_>>();
    bag.sort();
    bag
}

#[test]
fn frozen_eight_bags_are_independently_executable() {
    let frozen: Value = serde_json::from_str(include_str!("../docs/task-115-acceptance-freeze.json"))
        .expect("TASK-115 matrix");
    for case in frozen["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("id");
        let db = Connection::open_in_memory().expect("DuckDB");
        let constrained = case["variant"] == "pk_fk_constrained_sources";
        let ddl = if constrained {
            "CREATE TABLE t(id VARCHAR, k INTEGER, k2 INTEGER, tid INTEGER PRIMARY KEY);
             CREATE TABLE r(id VARCHAR PRIMARY KEY, k INTEGER, k2 INTEGER, tid INTEGER REFERENCES t(tid));
             CREATE TABLE l(id VARCHAR PRIMARY KEY, k INTEGER, k2 INTEGER, a INTEGER);"
        } else {
            "CREATE TABLE t(id VARCHAR, k INTEGER, k2 INTEGER, tid INTEGER);
             CREATE TABLE r(id VARCHAR, k INTEGER, k2 INTEGER, tid INTEGER);
             CREATE TABLE l(id VARCHAR, k INTEGER, k2 INTEGER, a INTEGER);"
        };
        db.execute_batch(ddl).expect("schema and constraints");
        for source in ["t", "r", "l"] {
            insert_rows(
                &db,
                source,
                case["physical_source_bags"][source].as_array().expect("physical rows"),
            );
        }
        let composite = case["variant"] == "composite_nullable_keys";
        let truth = if composite {
            "l.a>0 AND l.k=r.k AND l.k2=r.k2 AND r.k=t.k AND r.k2=t.k2"
        } else {
            "l.a>0 AND l.k=r.k AND r.k=t.k"
        };
        assert_eq!(
            actual_identity_bag(&db, truth, true),
            identities(&case["selected_physical_triplets"]),
            "{id}: SQL TRUE source triplets"
        );
        assert_eq!(
            actual_identity_bag(&db, truth, false),
            identities(&case["rejected_not_true_triplets"]),
            "{id}: SQL NOT TRUE source triplets"
        );
        let statements = case["sql"].as_str().expect("SQL").split(';').collect::<Vec<_>>();
        let query = statements.last().expect("terminal");
        for statement in statements.iter().take(statements.len() - 1) {
            db.execute_batch(statement).expect("produce stage from complete sources");
        }
        let grouped = case["variant"] == "downstream_group_count";
        let mut result = db.prepare(query).expect("original terminal SQL");
        let actual = if grouped {
            result.query_map([], |row| Ok(serde_json::json!({
                "k": row.get::<_, Option<i64>>(0)?,
                "n": row.get::<_, i64>(1)?,
            }))).expect("group query").map(|r| r.expect("group")).collect::<Vec<_>>()
        } else {
            result.query_map([], |row| Ok(serde_json::json!({
                "k": row.get::<_, Option<i64>>(0)?,
            }))).expect("join query").map(|r| r.expect("join")).collect::<Vec<_>>()
        };
        let mut expected = case["expected_terminal_bag"].as_array().expect("terminal bag")
            .iter().map(|item| {
                if grouped {
                    serde_json::json!({"k":item["k"],"n":item["n"]})
                } else {
                    serde_json::json!({"k":item["k"]})
                }
            }).collect::<Vec<_>>();
        let mut actual = actual;
        actual.sort_by_key(Value::to_string);
        expected.sort_by_key(Value::to_string);
        assert_eq!(actual, expected, "{id}: SQL output exactly matches frozen bag");
    }
}
